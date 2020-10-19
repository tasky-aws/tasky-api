use std::sync::Arc;

use futures::future::join_all;
use rusoto_core::Region;
use rusoto_credential::StaticProvider;
use rusoto_s3::{GetObjectOutput, GetObjectRequest, S3, S3Client};
use serde_json::Value;
use tokio::io::AsyncReadExt;

use crate::aws::client::HttpClient;
use crate::aws::credentials::Credentials;

pub fn build_s3_client(client: Arc<HttpClient>, creds: Credentials) -> S3Client {
    let cred_provider = StaticProvider::new(
        creds.aws_access_key,
        creds.aws_secret_key,
        Some(creds.aws_sts_token),
        None,
    );
    S3Client::new_with(client, cred_provider, Region::EuWest1)
}

pub async fn get_object(client: S3Client, bucket_file: &str) -> GetObjectOutput {
    let (bucket, key) = build_bucket_path(&bucket_file);
    let request = GetObjectRequest {
        bucket,
        if_match: None,
        if_modified_since: None,
        if_none_match: None,
        if_unmodified_since: None,
        key,
        part_number: None,
        range: None,
        request_payer: None,
        response_cache_control: None,
        response_content_disposition: None,
        response_content_encoding: None,
        response_content_language: None,
        response_content_type: None,
        response_expires: None,
        sse_customer_algorithm: None,
        sse_customer_key: None,
        sse_customer_key_md5: None,
        version_id: None,
    };
    client.clone().get_object(request).await.unwrap()
}

fn build_bucket_path(bucket_file: &str) -> (String, String) {
    let arguments: Vec<&str> = bucket_file.split(':').collect();
    if arguments.is_empty() {
        panic!("Bucket file was not in the format of 'BUCKET_NAME:BUCKET_KEY'")
    }
    log::debug!("Arguments: {:?}", arguments);
    let (bucket, key) = (arguments[0], arguments[1]);
    (bucket.replace(":", ""), key.replace(":", ""))
}


pub fn build_s3_clients(creds: Vec<Credentials>, client: &Arc<HttpClient>) -> Vec<S3Client> {
    creds
        .into_iter()
        .map(|creds| build_s3_client(client.clone(), creds))
        .collect()
}

fn deserialise_objects(objects: Vec<String>) -> Vec<Value> {
    let objects = objects
        .into_iter()
        .map(|object| serde_json::from_str(&object).expect("Unable to read object"))
        .collect();
    trace!("Objects json: {:#?}", objects);
    objects
}

async fn _get_objects_results(objects: Vec<GetObjectOutput>) -> Vec<String> {
    let all_objects_res_futures = objects.into_iter().map(|res| async {
        let stream = res.body.unwrap();
        let mut buffer = Vec::new();
        let mut reader = stream.into_async_read();
        reader.read_to_end(&mut buffer).await.expect("Failed to read to the end of the buffer");

        String::from_utf8(buffer)
    });

    let all_objects_results = join_all(all_objects_res_futures).await;
    let all_objects_results = all_objects_results
        .into_iter()
        .map(|c| c.unwrap())
        .collect();
    trace!("Resulting S3 Objects: {:?}", all_objects_results);
    all_objects_results
}

async fn _get_objects(clients: Vec<S3Client>, bucket_file: &String) -> Vec<GetObjectOutput> {
    let get_object_futures = clients
        .into_iter()
        .map(|client| get_object(client, &bucket_file));
    join_all(get_object_futures).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_s3_read() {}
}
