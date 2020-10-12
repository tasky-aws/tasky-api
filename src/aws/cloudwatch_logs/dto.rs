use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventType {
    LOG,
    TOKEN,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventResponse {
    /// <p>The ID of the event.</p>
    #[serde(rename = "eventType")]
    pub event_type: EventType,
    /// <p>The ID of the event.</p>
    #[serde(rename = "eventId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// <p>The time the event was ingested, expressed as the number of milliseconds after Jan 1, 1970 00:00:00 UTC.</p>
    #[serde(rename = "ingestionTime")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingestion_time: Option<i64>,
    /// <p>The name of the log stream to which this event belongs.</p>
    #[serde(rename = "logStreamName")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_stream_name: Option<String>,
    /// <p>The data contained in the log event.</p>
    #[serde(rename = "message")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// <p>The time the event occurred, expressed as the number of milliseconds after Jan 1, 1970 00:00:00 UTC.</p>
    #[serde(rename = "timestamp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogsOptions {
    pub role_arn: String,
    pub log_group: String,
    pub log_stream_name_prefix: Option<String>,
    pub next_token: Option<String>,
    pub limit: Option<i64>,
    pub filter_pattern: Option<String>,
    pub start_time_utc_millis: Option<i64>,
    pub end_time_utc_millis: Option<i64>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_logs_options_serialises() {}

    #[test]
    fn test_events_response_serialises() {}

    #[test]
    fn test_event_type_serialises() {}
}

