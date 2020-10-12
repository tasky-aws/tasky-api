#![feature(try_trait)]

#[macro_use]
extern crate log;


use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use warp::{Filter};
use warp::hyper::Method;

use aws::ecs::get_ecs_filter;

use crate::aws::cloudwatch_logs::{get_logs_events_filter, get_logs_filter};
use crate::aws::cloudwatch_logs::dto::LogsOptions;
use crate::aws::dto::AwsRequest;
use crate::aws::manager::setup_default_manager;
use error::handle_rejection;
use crate::notifications::{subscriber_connected, build_fan_notifications, NotUtf8};

mod aws;
mod error;
mod notifications;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();


    let subscribers = Arc::new(Mutex::new(HashMap::new()));
    let subscribers = warp::any().map(move || subscribers.clone());

    let cors_headers = vec![
        "User-Agent",
        "Sec-Fetch-Mode",
        "Referer",
        "Origin",
        "Access-Control-Request-Method",
        "Access-Control-Request-Headers",
        "content-type",
        "log_group",
        "role_arn"
    ];

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(cors_headers)
        .allow_methods(&[Method::GET, Method::POST]);

    let ecs = warp::path("ecs")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json::<AwsRequest>())
        .and_then(get_ecs_filter);

    let log_stream = warp::path("logs")
        .and(warp::path("events"))
        .and(warp::get())
        .and(warp::query::<LogsOptions>())
        .and_then(get_logs_events_filter);

    let logs = warp::path("logs")
        .and(warp::get())
        .and(warp::query::<LogsOptions>())
        .and_then(get_logs_filter);

    let bootstrap_config = warp::path("config")
        .and(warp::post())
        .and_then(setup_default_manager);

    let notify = warp::path("notify")
        .and(warp::post())
        .and(warp::body::content_length_limit(500))
        .and(
            warp::body::bytes()
                .and_then(|body: bytes::Bytes| async move {
                    std::str::from_utf8(&body)
                        .map(String::from)
                        .map_err(|_e| warp::reject::custom(NotUtf8))
                }),
        )
        .and(subscribers.clone())
        .map(|msg, users| {
            build_fan_notifications(msg, &users);
            warp::reply()
        });

    let notifications = warp::path("notifications")
        .and(warp::get())
        .and(subscribers)
        .map(|subscribers| {
            // reply using server-sent events
            let stream = subscriber_connected(subscribers);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

    warp::serve(
        ecs.or(logs)
            .or(log_stream)
            .or(bootstrap_config)
            .or(notify)
            .or(notifications)
            .with(cors)
            .recover(handle_rejection)
    )
        .run(([127, 0, 0, 1], 3030)).await;
}