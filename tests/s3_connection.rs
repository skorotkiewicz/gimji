use gimji::storage::S3ConnectionSettings;

#[test]
fn s3_connection_settings_reject_missing_endpoint() {
    let settings = S3ConnectionSettings {
        endpoint_url: String::new(),
        region: "us-east-1".to_owned(),
        bucket: String::new(),
        access_key_id: "minioadmin".to_owned(),
        secret_access_key: "minioadmin".to_owned(),
    };

    let error = settings.validate_for_connection().unwrap_err();

    assert!(error.to_string().contains("endpoint URL"));
}

#[test]
#[ignore = "requires a reachable MinIO/S3 endpoint"]
fn s3_connection_test_uses_s3_endpoint_when_no_bucket_is_selected() {
    let settings = S3ConnectionSettings {
        endpoint_url: std::env::var("GIMJI_S3_ENDPOINT")
            .unwrap_or_else(|_| "http://192.168.0.125:9000".to_owned()),
        region: std::env::var("GIMJI_S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
        bucket: String::new(),
        access_key_id: std::env::var("GIMJI_S3_ACCESS_KEY")
            .unwrap_or_else(|_| "minioadmin".to_owned()),
        secret_access_key: std::env::var("GIMJI_S3_SECRET_KEY")
            .unwrap_or_else(|_| "minioadmin".to_owned()),
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");

    runtime
        .block_on(settings.test_connection())
        .expect("connect to S3 endpoint");
}
