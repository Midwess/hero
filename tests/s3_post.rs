#![cfg(all(feature = "s3", not(target_family = "wasm")))]

use hero::s3::{ObjectPath, PostUploadOptions, PresignedPost, S3Config};
use std::time::{Duration, SystemTime};

fn client() -> hero::s3::S3Client {
    S3Config {
        region: "us-east-1".into(),
        access_key_id: "admin".into(),
        secret_access_key: "shhshhshh".into(),
        endpoint_url: Some(std::env::var("HERO_TEST_S3_ENDPOINT").unwrap_or("http://127.0.0.1:9000".into())),
        force_path_style: true,
    }
    .build_client()
}

fn options() -> PostUploadOptions {
    PostUploadOptions {
        expires_at: SystemTime::now() + Duration::from_secs(300),
        min_content_length: 4,
        max_content_length: 8,
        content_type: Some("application/octet-stream".into()),
    }
}

#[tokio::test]
async fn post_targets_bucket_and_preserves_exact_key() {
    let post = client().sign_post_upload(&ObjectPath::new("assets", "a /雪+%?#.bin"), &options()).await.unwrap();
    assert_eq!(post.url, "http://127.0.0.1:9000/assets/");
    assert_eq!(post.fields["key"], "a /雪+%?#.bin");
    let mut virtual_config = S3Config {
        region: "us-east-1".into(),
        access_key_id: "admin".into(),
        secret_access_key: "shhshhshh".into(),
        endpoint_url: Some("https://storage.example.com/base/".into()),
        force_path_style: false,
    };
    let post = virtual_config
        .build_client()
        .sign_post_upload(&ObjectPath::new("assets", "key"), &options())
        .await
        .unwrap();
    assert_eq!(post.url, "https://assets.storage.example.com/base/");
    virtual_config.force_path_style = true;
    let post = virtual_config
        .build_client()
        .sign_post_upload(&ObjectPath::new("assets", "key"), &options())
        .await
        .unwrap();
    assert_eq!(post.url, "https://storage.example.com/base/assets/");
}

#[tokio::test]
async fn temporary_credentials_are_included_and_bound_to_policy_lifetime() {
    use aws_sdk_s3::config::Credentials;
    use base64::{engine::general_purpose::STANDARD, Engine};
    let expiry = SystemTime::now() + Duration::from_secs(600);
    let client = hero::s3::S3Client::with_credentials_provider(
        client().raw_client().clone(),
        Credentials::new(
            "temporary",
            "temporary-secret",
            Some("session-token".into()),
            Some(expiry),
            "test",
        ),
    );
    let path = ObjectPath::new("assets", "key");
    let post = client.sign_post_upload(&path, &options()).await.unwrap();
    assert_eq!(post.fields["x-amz-security-token"], "session-token");
    let policy: serde_json::Value = serde_json::from_slice(&STANDARD.decode(&post.fields["policy"]).unwrap()).unwrap();
    let conditions = policy["conditions"].as_array().unwrap();
    assert!(conditions.contains(&serde_json::json!({"x-amz-security-token": "session-token"})));
    assert!(conditions.contains(&serde_json::json!(["content-length-range", 4, 8])));
    let mut too_long = options();
    too_long.expires_at = expiry + Duration::from_secs(1);
    assert!(client.sign_post_upload(&path, &too_long).await.is_err());
}

async fn upload(post: &PresignedPost, bytes: &[u8]) -> reqwest::Response {
    let mut form = reqwest::multipart::Form::new();
    for (name, value) in &post.fields {
        form = form.text(name.clone(), value.clone());
    }
    form = form.part("file", reqwest::multipart::Part::bytes(bytes.to_vec()).file_name("test.bin"));
    reqwest::Client::new().post(&post.url).multipart(form).send().await.unwrap()
}

#[tokio::test]
#[ignore = "requires local S3-compatible storage; see docs/s3-post.md"]
async fn storage_enforces_post_authorization() {
    let client = client();
    let bucket = format!(
        "hero-post-{}",
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()
    );
    client.raw_client().create_bucket().bucket(&bucket).send().await.unwrap();
    let path = ObjectPath::new(&bucket, "allowed.bin");
    let post = client.sign_post_upload(&path, &options()).await.unwrap();
    assert!(upload(&post, b"1234").await.status().is_success());
    assert_eq!(client.download(&path).await.unwrap().unwrap(), b"1234");
    assert!(upload(&post, b"12345678").await.status().is_success());
    for bytes in [b"123".as_slice(), b"123456789".as_slice()] {
        assert!(upload(&post, bytes).await.status().is_client_error());
    }
    let mut tampered = post.clone();
    tampered.fields.insert("key".into(), "other.bin".into());
    assert!(upload(&tampered, b"1234").await.status().is_client_error());
    tampered = post.clone();
    tampered.fields.insert("Content-Type".into(), "text/plain".into());
    assert!(upload(&tampered, b"1234").await.status().is_client_error());
    tampered = post.clone();
    tampered.fields.get_mut("policy").unwrap().push('A');
    assert!(upload(&tampered, b"1234").await.status().is_client_error());
    let mut short = options();
    short.expires_at = SystemTime::now() + Duration::from_secs(1);
    let expired = client.sign_post_upload(&path, &short).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(upload(&expired, b"1234").await.status().is_client_error());
    assert_eq!(client.download(&path).await.unwrap().unwrap(), b"12345678");
    client.delete_object(&path).await.unwrap();
    client.raw_client().delete_bucket().bucket(bucket).send().await.unwrap();
}
