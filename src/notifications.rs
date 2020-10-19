
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use libzmq::{prelude::*, *};

use futures::{Stream, StreamExt, SinkExt};
use tokio::sync::mpsc;
use warp::{sse::ServerSentEvent, Rejection};
use anyhow::Context as AnyhowContext;

// Our global unique user id counter.
pub static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Message variants.
#[derive(Debug)]
pub enum Notification {
    Error(String),
    Message(String),
}

#[derive(Debug)]
pub struct NotUtf8;

impl warp::reject::Reject for NotUtf8 {}

pub type Subscribers = Arc<Mutex<HashMap<usize, mpsc::UnboundedSender<Notification>>>>;

pub fn subscriber_connected(
    subscribers: Subscribers,
) -> impl Stream<Item=Result<impl ServerSentEvent + Send + 'static, warp::Error>> + Send + 'static
{
    // Use a counter to assign a new unique ID for this user.
    let new_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    info!("New notification subscriber: {}, subscriber size: {}", new_id, subscribers.lock().unwrap().len());

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the event source...
    let (tx, rx) = mpsc::unbounded_channel();

    tx.send(Notification::Message(format!("User ID: {}", new_id)))
        // rx is right above, so this cannot fail
        .unwrap();

    // Save the sender in our list of connected users.
    subscribers.lock().unwrap().insert(new_id, tx);

    rx.map(|msg| match msg {
        Notification::Error(error) => Ok((warp::sse::event("ERROR"), warp::sse::data(error)).into_a()),
        Notification::Message(message) => Ok((warp::sse::event("INFO"), warp::sse::data(message)).into_b()),
    })
}

pub fn _build_notification(id: usize, msg: String, users: &Subscribers) {
    let new_msg = format!("<Subscriber#{}>: {}", id, msg);

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock()
        .unwrap()
        .retain(|uid, tx| {
            if id == *uid {
                // don't send to same user, but do retain
                true
            } else {
                // If not `is_ok`, the SSE stream is gone, and so don't retain
                tx.send(Notification::Message(new_msg.clone())).is_ok()
            }
        });
}

pub fn build_fan_notifications(msg: String, users: &Subscribers) {
    let new_msg = msg.to_string();

    // New message from this user, send it to everyone else (except same uid)...
    //
    // We use `retain` instead of a for loop so that we can reap any user that
    // appears to have disconnected.
    users.lock()
        .unwrap()
        .retain(|_, tx| {
            if true {
                if msg.contains("ERROR") || msg.contains("Error") {
                    tx.send(Notification::Error(serde_json::to_string(&new_msg.clone()).unwrap())).is_ok()
                } else {
                    tx.send(Notification::Message(serde_json::to_string(&new_msg.clone()).unwrap())).is_ok()
                }
            } else {
                false
            }
        });
}

pub async fn handle_message(bytes: bytes::Bytes, mut client: Client) -> Result<impl warp::Reply, Rejection> {
    let msg =  std::str::from_utf8(&bytes);
    info!("sending");
    if let Ok(msg) = msg {
        info!("msg was ok");
        if let Err(err) = client.send(msg) {
            error!("failed to send to internal proxy!, {}", err)
        }
    }

    Ok(warp::reply())
}
