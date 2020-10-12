use reject::Reject;
use serde::Serialize;
use warp::{reject, Rejection, Reply};
use hyper::StatusCode;
use anyhow::{anyhow, Error};
use std::error::Error as StandardError;
use std::convert::Infallible;

#[derive(Serialize)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

#[derive(Debug)]
pub struct ErrorWrapper {
    pub error: Error
}

impl Reject for ErrorWrapper {}

pub fn _extract_warp_err<T>(value: Result<T, Error>) -> Result<T, Rejection> {
    match value {
        Ok(value) => Ok(value),
        Err(err) => {
            log::error!("Failed to match result: `{}`", err);
            Err(reject::custom(ErrorWrapper {
                error: err
            }))
        }
    }
}

/// To use this you need to import below as i havent gotten around to it yet
///
/// use crate::error::ErrorWrapper;
/// use crate::extract_rejection;
/// use warp::reject;
/// use warp::Rejection;
#[macro_export]
macro_rules! extract_rejection {
    ($v:expr) => {
        (
                match $v {
                    Ok(value) => Ok(value),
                    Err(err) => {
                        log::error!("Failed to match result: `{}`", err);
                        Err(reject::custom(ErrorWrapper {
                            error: err
                        }))
                    }
                }
        )
    };
}// This function receives a `Rejection` and tries to return a custom
// value, otherwise simply passes the rejection along.
pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message: String;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND".to_string();
    } else if let Some(err) = err.find::<ErrorWrapper>() {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        let formatted_message = format!("{:?}", err.error);
        if formatted_message.contains("token") {
            let client = reqwest::Client::new();
            let result = client.post("http://localhost:3030/notify")
                .body(formatted_message.clone())
                .send()
                .await
                .map_err(|err| anyhow::anyhow!(err));
            if let Err(err) = result {
                error!("Failure sending failed notification, {}", err);
            };
        }
        message = formatted_message;
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        // This error happens if the body could not be deserialized correctly
        // We can use the cause to analyze the error and customize the error message
        message = match e.source() {
            Some(cause) => {
                if cause.to_string().contains("denom") {
                    "FIELD_ERROR: denom".to_string()
                } else {
                    "BAD_REQUEST".to_string()
                }
            }
            None => "BAD_REQUEST".to_string(),
        };
        code = StatusCode::BAD_REQUEST;
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        // We can handle a specific error, here METHOD_NOT_ALLOWED,
        // and render it however we want
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "METHOD_NOT_ALLOWED".to_string();
    } else {
        // We should have expected this... Just log and say its a 500
        eprintln!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION".to_string();
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}