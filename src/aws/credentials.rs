use std::result::Result;
use std::sync::Arc;

use futures::future::join_all;
use rusoto_core::region::Region;
use rusoto_core::request::BufferedHttpResponse;
use rusoto_core::RusotoError;
use rusoto_credential::StaticProvider;
use rusoto_sts::{AssumeRoleError, AssumeRoleRequest, AssumeRoleResponse, Sts, StsClient};
use anyhow::{anyhow, Error, Context};

use crate::aws::client::HttpClient;
use crate::aws::dto::AwsMessage;
use crate::aws::manager::Config;

#[derive(Debug)]
pub struct Credentials {
    pub aws_access_key: String,
    pub aws_secret_key: String,
    pub aws_sts_token: String,
}

pub async fn assume_role(
    config: &Config,
    client: Arc<HttpClient>,
    role_arn: &str,
) -> Result<Credentials, Error> {
    debug!("Assuming role with config: {:?} and role_arn: {:?}", config, role_arn);
    if config.is_token_valid() {
        let cred_provider = build_static_provider(config)?;
        let response = request_assume_role(client, role_arn, cred_provider).await?;
        let credentials = extract_credentials(response)?;

        Ok(Credentials {
            aws_access_key: credentials.access_key_id,
            aws_secret_key: credentials.secret_access_key,
            aws_sts_token: credentials.session_token,
        })
    } else {
        Err(anyhow!("Token is not valid"))
    }
}

fn extract_credentials(assume_role_res: AssumeRoleResponse) -> Result<rusoto_sts::Credentials, Error> {
    Ok(assume_role_res
        .clone()
        .credentials
        .ok_or_else(|| {
            anyhow!(format!(
                "Could not create an assume role from the response `{:?}`",
                assume_role_res
            ))
        })
        .with_context(|| "Missing credentials from assume role")?)
}

fn build_static_provider(config: &Config) -> Result<StaticProvider, Error> {
    Ok(StaticProvider::new(
        config
            .aws_temp_access_key_id
            .clone()
            .ok_or_else(|| anyhow!("aws_temp_access_key_id is not set"))?,
        config
            .aws_temp_secret_access_key
            .clone()
            .ok_or_else(|| anyhow!("aws_temp_secret_access_keys not set"))?,
        config.aws_session_token.clone(),
        None,
    ))
}

async fn request_assume_role(client: Arc<HttpClient>, role_arn: &str, cred_provider: StaticProvider) -> Result<AssumeRoleResponse, Error> {
    let sts_client = StsClient::new_with(client, cred_provider, Region::EuWest1);

    debug!("Assuming role for arn: {}", role_arn.to_string());

    let assume_role_request = build_assume_role_request(role_arn);

    let response = sts_client
        .assume_role(assume_role_request)
        .await;

    match response {
        Ok(value) => Ok(value),
        Err(err) => Err(match_rusoto_errors(err)),
    }
}

fn build_assume_role_request(role_arn: &str) -> AssumeRoleRequest {
    AssumeRoleRequest {
        role_arn: role_arn.to_owned(),
        role_session_name: "dummy".to_owned(),
        ..Default::default()
    }
}

fn match_rusoto_errors(err: RusotoError<AssumeRoleError>) -> Error {
    match err {
        RusotoError::Service(err) => anyhow!(format!("{}", err)),
        RusotoError::HttpDispatch(err) => anyhow!(format!("{}", err)),
        RusotoError::Credentials(err) => anyhow!(format!("{}", err)),
        RusotoError::Validation(err) => anyhow!(err),
        RusotoError::ParseError(err) => anyhow!(err),
        RusotoError::Unknown(err) => parse_aws_xml(err),
        RusotoError::Blocking => anyhow!("There was a blocking issue assuming roles"),
    }
}

fn parse_aws_xml(err: BufferedHttpResponse) -> Error {
    let doc_str = format!("{:?}", err.body);
    if let Some(idl_ix) = doc_str.find("<Message>") {
        let aws_message: Result<AwsMessage, serde_xml_rs::Error> = serde_xml_rs::from_str(&doc_str[idl_ix..]);
        match aws_message {
            Ok(value) => anyhow!(value.message),
            Err(_) => anyhow!(format!("Failed to read xml from document: {}", doc_str))
        }
    } else {
        anyhow!(format!("Aws returned body: {:?}", err.body))
    }
}

pub async fn build_credential(
    role_arn: &str,
    config: &Config,
    client: &Arc<HttpClient>,
) -> Result<Credentials, Error> {
    assume_role(&config, client.clone(), &role_arn).await
}

pub async fn _build_credentials(
    role_arns: Vec<String>,
    config: &Config,
    client: &Arc<HttpClient>,
) -> Vec<Credentials> {
    let get_creds_futures = role_arns
        .iter()
        .map(|role_arn| build_credential(&role_arn, &config, &client));
    let get_creds = join_all(get_creds_futures).await;
    get_creds
        .into_iter()
        .filter(|res| res.is_ok())
        .map(|res| res.unwrap())
        .collect::<Vec<Credentials>>()
}

fn _iterate_credentials<T, F>(
    credentials: Vec<Credentials>,
    base_client: &Arc<HttpClient>,
    action: F,
) -> Vec<T>
    where F: Fn(&Arc<HttpClient>, Credentials) -> T, {
    credentials
        .into_iter()
        .map(|creds| action(&base_client.clone(), creds))
        .collect()
}


