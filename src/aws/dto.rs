use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AwsRequest {
    pub role_arn: String
}

#[derive(Deserialize, Debug)]
pub struct AwsMessage {
    #[serde(rename = "$value")]
    pub message: String,
}

