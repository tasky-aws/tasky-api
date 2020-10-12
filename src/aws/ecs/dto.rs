use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceResponse {
    /// <p>The Unix timestamp for when the service was created.</p>
    #[serde(rename = "createdAt")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<f64>,
    /// <p>The principal that created the service.</p>
    #[serde(rename = "createdBy")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// <p>The desired number of instantiations of the task definition to keep running on the service. This value is specified when the service is created with <a>CreateService</a>, and it can be modified with <a>UpdateService</a>.</p>
    #[serde(rename = "desiredCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_count: Option<i64>,
    /// <p>The period of time, in seconds, that the Amazon ECS service scheduler ignores unhealthy Elastic Load Balancing target health checks after a task has first started.</p>
    #[serde(rename = "healthCheckGracePeriodSeconds")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_grace_period_seconds: Option<i64>,
    #[serde(rename = "pendingCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_count: Option<i64>,
    /// <p>The number of tasks in the cluster that are in the <code>RUNNING</code> state.</p>
    #[serde(rename = "runningCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running_count: Option<i64>,
    /// <p>The ARN that identifies the service. The ARN contains the <code>arn:aws:ecs</code> namespace, followed by the Region of the service, the AWS account ID of the service owner, the <code>service</code> namespace, and then the service name. For example, <code>arn:aws:ecs:region:012345678910:service/my-service</code>.</p>
    #[serde(rename = "serviceArn")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_arn: Option<String>,
    /// <p>The name of your service. Up to 255 letters (uppercase and lowercase), numbers, and hyphens are allowed. Service names must be unique within a cluster, but you can have similarly named services in multiple clusters within a Region or across multiple Regions.</p>
    #[serde(rename = "serviceName")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(rename = "taskDefinition")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_definition: Option<String>,
    #[serde(rename = "tasks")]
    pub tasks: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterResponse {
    #[serde(rename = "activeServicesCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_services_count: Option<i64>,
    #[serde(rename = "clusterArn")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_arn: Option<String>,
    #[serde(rename = "clusterName")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
    #[serde(rename = "pendingTasksCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_tasks_count: Option<i64>,
    #[serde(rename = "runningTasksCount")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running_tasks_count: Option<i64>,
    #[serde(rename = "services")]
    pub services: Vec<ServiceResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseWrapper {
    pub(crate) clusters: Vec<ClusterResponse>
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_wrapper_serialises() {}

    #[test]
    fn test_cluster_response_serialises() {}

    #[test]
    fn test_service_response_serialises() {}
}