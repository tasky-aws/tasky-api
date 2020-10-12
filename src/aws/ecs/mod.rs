use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use resiter::GetOks;
use rusoto_core::{Region, RusotoError};
use rusoto_credential::StaticProvider;
use rusoto_ecs::{Cluster, DescribeClustersError, ListClustersError};
use rusoto_ecs::DescribeClustersRequest;
use rusoto_ecs::DescribeClustersResponse;
use rusoto_ecs::DescribeServicesRequest;
use rusoto_ecs::Ecs;
use rusoto_ecs::EcsClient;
use rusoto_ecs::ListClustersRequest;
use rusoto_ecs::ListClustersResponse;
use rusoto_ecs::ListServicesRequest;
use rusoto_ecs::ListServicesResponse;
use rusoto_ecs::ListTasksRequest;
use rusoto_ecs::ListTasksResponse;
use rusoto_ecs::Service;
use warp::reject;
use warp::Rejection;

use crate::aws::client;
use crate::aws::client::HttpClient;
use crate::aws::credentials::{build_credential, Credentials};
use crate::aws::dto::AwsRequest;
use crate::aws::ecs::dto::{ClusterResponse, ResponseWrapper, ServiceResponse};
use crate::aws::manager::Config;
use crate::error::ErrorWrapper;
use crate::extract_rejection;
use anyhow::{anyhow, Error};

mod dto;


pub async fn get_ecs_filter(request: AwsRequest) -> Result<impl warp::Reply, Rejection> {
    let config = extract_rejection!(Config::load())?;
    let client = Arc::new(extract_rejection!(client::new_client())?);

    let creds = extract_rejection!(build_credential(&request.role_arn, &config, &client).await)?;

    let ecs_client = build_ecs_client(client.clone(), creds);

    let query = extract_rejection!(query_ecs(ecs_client).await)?;

    let result = extract_rejection!(map_to_response(query.0, query.1, query.2))?;
    Ok(warp::reply::json(&result))
}

async fn query_ecs(client: EcsClient) -> Result<(DescribeClustersResponse, HashMap<String, Vec<Service>, RandomState>, HashMap<String, ListTasksResponse, RandomState>), Error> {
    let client = Arc::new(client);
    let list_clusters = get_clusters(&client.clone()).await?;
    let cluster_arns = list_clusters.cluster_arns.clone();
    let cluster_arns_cloned = list_clusters.cluster_arns.clone();

    let client_cloned = client.clone();
    let clusters_described = tokio::task::spawn(async move {
        describe_clusters(&client_cloned, &cluster_arns).await
    });

    let client_cloned = client.clone();
    let services = tokio::task::spawn(async move {
        get_services(&client_cloned, &cluster_arns_cloned).await // Now have services mapped to cluster ids
    });

    let clusters_described = clusters_described.await??; // Can take name, pending and running in here

    let services = services.await??; // Now have services mapped to cluster ids
    let services_described = describe_services(&client, services.clone()).await;
    let tasks = get_tasks(&client, services_described.clone()).await?;

    Ok((clusters_described, services_described, tasks))
}

pub fn build_ecs_client(client: Arc<HttpClient>, creds: Credentials) -> EcsClient {
    let cred_provider = StaticProvider::new(
        creds.aws_access_key,
        creds.aws_secret_key,
        Some(creds.aws_sts_token),
        None,
    );
    EcsClient::new_with(client, cred_provider, Region::EuWest1) //TODO update region
}

fn map_to_response(
    clusters_described: DescribeClustersResponse,
    services_described: HashMap<String, Vec<Service>>,
    tasks: HashMap<String, ListTasksResponse>,
) -> Result<ResponseWrapper, Error> {
    let cluster_map: HashMap<String, Cluster> = build_cluster_map(clusters_described)?;
    let mut clusters: Vec<ClusterResponse> = cluster_map
        .keys()
        .filter(|key| !key.contains("default"))
        .map(|cluster_id| {
            if let Some(cluster) = cluster_map.get(cluster_id) {
                Ok((cluster_id, cluster))
            } else {
                Err(anyhow!("Failed to read cluster arn"))
            }
        })
        .oks()
        .map(|(cluster_id, cluster)| {
            if let Ok(services) = iterate_services_described(&services_described, &tasks, cluster_id) {
                Ok((cluster, services))
            } else {
                Err(anyhow!("Failed to iterate services"))
            }
        })
        .oks()
        .map(|(cluster, services)| {
            ClusterResponse {
                active_services_count: cluster.active_services_count,
                cluster_arn: cluster.clone().cluster_arn,
                cluster_name: cluster.clone().cluster_name,
                pending_tasks_count: cluster.pending_tasks_count,
                running_tasks_count: cluster.running_tasks_count,
                services,
            }
        })
        .collect();
    clusters.sort_by(|a, b| a.cluster_name.cmp(&b.cluster_name));
    let response = ResponseWrapper {
        clusters
    };
    Ok(response)
}

fn build_cluster_map(clusters_described: DescribeClustersResponse) -> Result<HashMap<String, Cluster>, Error> {
    Ok(clusters_described.clusters.ok_or_else(|| anyhow!("Failed to read clusters"))?
        .into_iter()
        .map(|cluster| {
            if let Some(value) = cluster.clone().cluster_arn {
                Ok((value, cluster))
            } else {
                Err(anyhow!("Failed to read cluster arn"))
            }
        })
        .oks()
        .collect())
}

fn iterate_services_described(
    services_described: &HashMap<String, Vec<Service>>,
    tasks: &HashMap<String, ListTasksResponse>,
    cluster_id: &str,
) -> Result<Vec<ServiceResponse>, Error> {
    let mut services: Vec<ServiceResponse> = services_described.get(cluster_id)
        .ok_or_else(|| anyhow!(format!("Failed to receive cluster {} from services {:?}", cluster_id, services_described)))?
        .iter()
        .map(|service| {
            let service = service.clone();
            if let Some(service_arn) = service.service_arn.clone() {
                Ok((service_arn, service))
            } else {
                Err(anyhow!("Failed to read service arn"))
            }
        })
        .oks()
        .map(|(service_arn, service)| {
            ServiceResponse {
                created_at: service.created_at,
                created_by: service.created_by,
                desired_count: service.desired_count,
                health_check_grace_period_seconds: service.health_check_grace_period_seconds,
                pending_count: service.pending_count,
                running_count: service.running_count,
                service_arn: service.service_arn,
                service_name: service.service_name,
                task_definition: service.task_definition,
                tasks: tasks.get(&service_arn).unwrap().task_arns.as_ref().unwrap().clone(),
            }
        }).collect();
    services.sort_by(|a, b| a.service_name.cmp(&b.service_name));
    Ok(services)
}

pub async fn _iterate_clients(clients: &[EcsClient]) -> Result<Vec<ListClustersResponse>, Error> {
    let clusters = clients
        .iter()
        .map(|client| async move {
            get_clusters(&client).await
        });
    let joined_results = join_all(clusters).await
        .into_iter()
        .oks()
        .collect();
    Ok(joined_results)
}

pub async fn get_clusters(client: &EcsClient) -> Result<ListClustersResponse, RusotoError<ListClustersError>> {
    client.list_clusters(ListClustersRequest {
        max_results: None,
        next_token: None,
    }).await
}

pub async fn describe_clusters(client: &EcsClient, clusters: &Option<Vec<String>>) -> Result<DescribeClustersResponse, RusotoError<DescribeClustersError>> {
    let owned_clusters = clusters.to_owned();
    let request = DescribeClustersRequest {
        clusters: owned_clusters,
        include: None,
    };
    Ok(client.describe_clusters(request).await?)
}


pub async fn get_services(client: &EcsClient, clusters: &Option<Vec<String>>) -> Result<HashMap<String, ListServicesResponse>, Error> {
    let owned = clusters.to_owned().ok_or_else(|| anyhow!("Failed to take ownership of clusters"))?;
    let services = owned
        .iter()
        .map(|cluster| async move {
            let cluster = cluster.to_owned();
            (cluster.clone(), client.list_services(ListServicesRequest {
                cluster: Some(cluster),
                launch_type: None,
                max_results: None,
                next_token: None,
                scheduling_strategy: None,
            }).await.unwrap())
        });
    Ok(join_all(services).await
        .into_iter()
        .collect())
}

pub async fn get_tasks(client: &EcsClient, clusters: HashMap<String, Vec<Service>>) -> Result<HashMap<String, ListTasksResponse>, Error> {
    let tasks = clusters.values().flatten()
        .map(|service| async move {
            let cluster = service.clone().cluster_arn.unwrap();
            (service.service_arn.clone().unwrap(), client.list_tasks(ListTasksRequest {
                cluster: Some(cluster),
                container_instance: None,
                desired_status: None,
                family: None,
                launch_type: None,
                max_results: None,
                next_token: None,
                service_name: service.clone().service_name,
                started_by: None,
            }).await.unwrap())
        });
    Ok(join_all(tasks).await
        .into_iter()
        .collect())
}

pub async fn describe_services(client: &EcsClient, services: HashMap<String, ListServicesResponse>) -> HashMap<String, Vec<Service>> {
    let clusters_service_arns = services.iter()
        .filter(|(_, list_services)| !list_services.service_arns.as_ref().unwrap().is_empty())
        .map(|(cluster, list_services)| {
            let cluster = cluster.to_owned();

            if let Some(service_arns) = list_services.clone().service_arns {
                Ok((cluster, service_arns))
            } else {
                Err(anyhow!("Could not unwrap service arns"))
            }
        });

    let clusters_services = clusters_service_arns
        .oks()
        .map(|(cluster, service_arns)| async move {
            let cluster = cluster.to_owned();
            let services = service_arns;

            let result = client.describe_services(DescribeServicesRequest {
                cluster: Some(cluster.clone()),
                include: None,
                services,
            }).await;
            if let Ok(services) = result {
                Ok((cluster.clone(), services))
            } else {
                Err(anyhow!("Failed to describe services"))
            }
        });

    join_all(clusters_services).await
        .into_iter()
        .oks()
        .map(|(cluster_id, response)| {
            if let Some(services) = response.services {
                Ok((cluster_id, services))
            } else {
                Err(anyhow!("Failed to unwrap services from response"))
            }
        })
        .oks()
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_clusters() {}

    #[test]
    fn test_get_clusters_fail() {}

    #[test]
    fn test_describe_clusters() {}

    #[test]
    fn test_describe_clusters_fail() {}

    #[test]
    fn test_iterate_clients() {}

    #[test]
    fn test_iterate_clients_fail() {}

    #[test]
    fn test_get_services() {}

    #[test]
    fn test_get_services_fail() {}

    #[test]
    fn test_describe_services() {}

    #[test]
    fn test_describe_services_fail() {}

    #[test]
    fn test_get_tasks() {}

    #[test]
    fn test_get_tasks_fail() {}

    #[test]
    fn test_iterate_services_described() {}

    #[test]
    fn test_iterate_services_described_fail() {}

    #[test]
    fn test_build_cluster_map() {}

    #[test]
    fn test_map_to_responses() {}

    #[test]
    fn test_map_to_responses_fail() {}

    #[test]
    fn test_build_ecs_client() {}

    #[test]
    fn test_build_ecs_client_fail() {}

    #[test]
    fn test_query_ecs() {}

    #[test]
    fn test_query_ecs_fail() {}
}