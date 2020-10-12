#![feature(try_trait)]

#[macro_use]
extern crate log;


use std::collections::HashMap;
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context as AnyhowContext;
use futures::SinkExt;
use tokio::stream::StreamExt;
use tokio::time::{delay_for, Duration};
use warp::Filter;
use warp::hyper::Method;
use libzmq::{prelude::*, *};

use aws::ecs::get_ecs_filter;
use error::handle_rejection;

use crate::aws::cloudwatch_logs::{get_logs_events_filter, get_logs_filter};
use crate::aws::cloudwatch_logs::dto::LogsOptions;
use crate::aws::dto::AwsRequest;
use crate::aws::manager::setup_default_manager;
use crate::notifications::{build_fan_notifications, NotUtf8, subscriber_connected};
use libzmq::InprocAddr;
use std::thread;

mod test;
mod aws;
mod error;
mod notifications;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    pretty_env_logger::init();

    let inproc_addr: InprocAddr = InprocAddr::new_unique();
    let inproc_socket = ServerBuilder::new().bind(&inproc_addr).build()?;
    let subscriber_addr: TcpAddr = "eth0;192.168.1.1:5555".try_into()?;
    let publish_socket = ClientBuilder::new().connect(subscriber_addr).build()?;

    // Spawn the server thread.
    let handle = thread::spawn(move || -> Result<(), Error> {
        loop {
            let request = inproc_socket.recv_msg()?;
            let msg = request.to_str().unwrap_or_default();
            info!("Received a message on the inproc socket, msg: {}", msg);
            publish_socket.send(msg).unwrap_or_default();
        }
    });

    let client = ClientBuilder::new().connect(inproc_addr).build()?;



    // let publish_notification = context.socket(zmq::PUB)
    //     .with_context(|| "Failed to instantiate notification socket!").unwrap();
    // publish_notification
    //     .bind("tcp://127.0.0.1:3031").unwrap();
    //
    // let inproc_socket = context.socket(zmq::SUB)
    //     .with_context(|| "Failed to instantiate inproc socket!").unwrap();
    // inproc_socket
    //     .connect("inproc://internal_proxy").unwrap();
    // inproc_socket
    //     .set_subscribe(b"")
    //     .expect("failed setting subscription");
    //
    // std::thread::spawn(move || {
    //     while let Ok(msg) = inproc_socket.recv_msg(0) {
    //         info!("Received a message on the inproc socket");
    //         let result = publish_notification.send(msg, 0);
    //         if let Err(err) = result {
    //             error!("Error sending message to notification sender!, err {}", err)
    //         }
    //     }
    // });


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

    let socket_test = warp::path("sockettest")
        .and(warp::post())
        .and(warp::body::content_length_limit(500))
        .and(warp::body::bytes())
        .and_then(move r|msg| notifications::handle_message(msg, client.clone()));

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
            .or(socket_test)
            .or(notifications)
            .with(cors)
            .recover(handle_rejection)
    )
        .run(([127, 0, 0, 1], 3030)).await;

    // This will cause the server to fail with `InvalidCtx`.
    Ctx::global().shutdown();

    // Join with the thread.
    let err = handle.join().unwrap().unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidCtx);

    Ok(())
}