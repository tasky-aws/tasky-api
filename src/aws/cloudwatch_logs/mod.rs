use std::sync::Arc;

use anyhow::Error;
use chrono::{Duration, Utc};
use futures::Stream;
use rayon::{iter::ParallelIterator, prelude::IntoParallelIterator};
use rusoto_core::Region;
use rusoto_credential::StaticProvider;
use rusoto_logs::{
    CloudWatchLogs, CloudWatchLogsClient, FilterLogEventsRequest, FilterLogEventsResponse,
};
use tokio::stream::StreamExt;
use warp::reject;
use warp::reply::json;
use warp::sse::ServerSentEvent;
use warp::{sse, Rejection};

use dto::{EventResponse, LogsOptions};

use crate::aws::client;
use crate::aws::client::HttpClient;
use crate::aws::cloudwatch_logs::dto::EventType;
use crate::aws::credentials::{build_credential, Credentials};
use crate::aws::manager::Config;
use crate::error::ErrorWrapper;
use crate::extract_rejection;

pub mod dto;

pub async fn get_logs_events_filter(
    logs_options: LogsOptions,
) -> Result<impl warp::Reply, Rejection> {
    info!("Query params for logs filter: {:?}", logs_options);

    let config = extract_rejection!(Config::load())?;
    let client = Arc::new(extract_rejection!(client::new_client())?);
    let credentials =
        extract_rejection!(build_credential(&logs_options.role_arn, &config, &client).await)?;
    let client = build_logs_client(client.clone(), credentials);

    Ok(sse::reply(
        sse::keep_alive()
            .interval(std::time::Duration::from_secs(5))
            .text("Bumping due to interval")
            .stream(sse_events(client, logs_options).await),
    ))
}

pub async fn get_logs_filter(logs_options: LogsOptions) -> Result<impl warp::Reply, Rejection> {
    info!("Query params for logs filter: {:?}", logs_options);

    let config = extract_rejection!(Config::load())?;
    let client = Arc::new(extract_rejection!(client::new_client())?);
    let credentials =
        extract_rejection!(build_credential(&logs_options.role_arn, &config, &client).await)?;
    let client = build_logs_client(client.clone(), credentials);

    let mut logs: Vec<EventResponse> = extract_rejection!(get_logs(client, logs_options).await)?;
    logs.sort_by(|a, b| {
        a.timestamp
            .unwrap_or_default()
            .cmp(&b.timestamp.unwrap_or_default())
    });
    Ok(json(&logs))
}

async fn sse_events(
    client: CloudWatchLogsClient,
    options: LogsOptions,
) -> impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, warp::Error>> + Send + 'static
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let _result = tokio::task::spawn(async move {
        let query = extract_rejection!(query_logs(client.clone(), options).await);

        match query {
            Ok(value) => {
                let events = value.events.unwrap();

                let _token_sent = tx.send(
                    sse::json(EventResponse {
                        event_type: EventType::TOKEN,
                        event_id: None,
                        ingestion_time: None,
                        log_stream_name: None,
                        message: None,
                        timestamp: None,
                        token: value.next_token,
                    })
                    .boxed(),
                );

                for event in events {
                    tx.send(
                        sse::json(EventResponse {
                            event_type: EventType::LOG,
                            event_id: event.event_id,
                            ingestion_time: event.ingestion_time,
                            log_stream_name: event.log_stream_name,
                            message: event.message,
                            timestamp: event.timestamp,
                            token: None,
                        })
                        .boxed(),
                    )
                    .unwrap_or(());
                }
            }
            Err(err) => return Err(err),
        };
        Ok(())
    })
    .await;

    rx.map(Ok)
}

async fn get_logs(
    client: CloudWatchLogsClient,
    options: LogsOptions,
) -> Result<Vec<EventResponse>, Error> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let _result = tokio::task::spawn(async move {
        let query = extract_rejection!(query_logs(client.clone(), options).await);

        match query {
            Ok(value) => {
                let events = value.events.unwrap_or_default();

                let _token_sent = tx.send(EventResponse {
                    event_type: EventType::TOKEN,
                    token: value.next_token,
                    event_id: None,
                    ingestion_time: None,
                    log_stream_name: None,
                    message: None,
                    timestamp: None,
                });

                events.into_par_iter().for_each(|event| {
                    let result = tx.send(EventResponse {
                        event_type: EventType::LOG,
                        event_id: event.event_id,
                        ingestion_time: event.ingestion_time,
                        log_stream_name: event.log_stream_name,
                        message: event.message,
                        timestamp: event.timestamp,
                        token: None,
                    });
                    if let Err(err) = result {
                        error!("Some error sending event over channel {}", err)
                    }
                });
            }
            Err(err) => return Err(err),
        };
        Ok(())
    })
    .await;

    rx.map(Ok).collect().await
}

async fn query_logs(
    client: CloudWatchLogsClient,
    options: LogsOptions,
) -> Result<FilterLogEventsResponse, Error> {
    let client = Arc::new(client);
    let request = FilterLogEventsRequest {
        filter_pattern: options.filter_pattern,
        limit: Some(options.limit.unwrap_or(100)),
        log_group_name: options.log_group.to_string(),
        log_stream_name_prefix: options.log_stream_name_prefix,
        log_stream_names: None,
        next_token: options.next_token,
        end_time: Some(
            options
                .end_time_utc_millis
                .unwrap_or_else(|| (Utc::now() + Duration::days(1)).timestamp_millis()),
        ),
        start_time: Some(
            options
                .start_time_utc_millis
                .unwrap_or_else(|| (Utc::now() - Duration::weeks(2)).timestamp_millis()),
        ),
    };
    info!("request {:?}", request);
    let logs = client.filter_log_events(request).await?;

    Ok(logs)
}

pub fn build_logs_client(client: Arc<HttpClient>, creds: Credentials) -> CloudWatchLogsClient {
    let cred_provider = StaticProvider::new(
        creds.aws_access_key,
        creds.aws_secret_key,
        Some(creds.aws_sts_token),
        None,
    );
    CloudWatchLogsClient::new_with(client, cred_provider, Region::EuWest1) //TODO update region
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_build_logs_client() {}

    #[test]
    fn test_query() {}

    #[test]
    fn test_get_logs() {}

    #[test]
    fn test_sse_events() {}
}
