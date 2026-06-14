use gimji::models::TabContent;
use gimji::storage::S3ConnectionSettings;
use gimji::storage::Workspace;

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

#[test]
fn s3_backup_requires_bucket_name() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let workspace = Workspace::create(temp_dir.path()).expect("workspace");
    let settings = S3ConnectionSettings {
        endpoint_url: "http://192.168.0.125:9000".to_owned(),
        region: "us-east-1".to_owned(),
        bucket: String::new(),
        access_key_id: "minioadmin".to_owned(),
        secret_access_key: "minioadmin".to_owned(),
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");

    let error = runtime
        .block_on(settings.backup_workspace(&workspace))
        .unwrap_err();

    assert!(error.to_string().contains("bucket"));
}

#[test]
#[ignore = "requires a reachable MinIO/S3 endpoint and bucket"]
fn s3_backup_uploads_workspace_config_and_content_files() {
    let temp_dir = tempfile::tempdir().expect("temp workspace");
    let mut workspace = Workspace::create(temp_dir.path()).expect("workspace");
    let note_id = workspace.add_note("S3 Backup").expect("note");
    let tab_id = workspace
        .selected_tab_id()
        .expect("selected tab")
        .to_owned();
    workspace
        .save_tab_content(&tab_id, &TabContent::Markdown("backed up body".to_owned()))
        .expect("save content");
    let content_key = workspace.config().notes[0].tabs[0].file_name.clone();
    let settings = storage_bucket_settings();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");

    runtime
        .block_on(settings.backup_workspace(&workspace))
        .expect("backup workspace");

    let config_text = runtime
        .block_on(settings.read_text_object("config.json"))
        .expect("read config object");
    let content_text = runtime
        .block_on(settings.read_text_object(&content_key))
        .expect("read content object");

    assert!(config_text.contains(&note_id));
    assert_eq!(content_text, "backed up body");
}

fn storage_bucket_settings() -> S3ConnectionSettings {
    S3ConnectionSettings {
        endpoint_url: std::env::var("GIMJI_S3_ENDPOINT")
            .unwrap_or_else(|_| "http://192.168.0.125:9000".to_owned()),
        region: std::env::var("GIMJI_S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
        bucket: std::env::var("GIMJI_S3_BUCKET").unwrap_or_else(|_| "storage".to_owned()),
        access_key_id: std::env::var("GIMJI_S3_ACCESS_KEY")
            .unwrap_or_else(|_| "minioadmin".to_owned()),
        secret_access_key: std::env::var("GIMJI_S3_SECRET_KEY")
            .unwrap_or_else(|_| "minioadmin".to_owned()),
    }
}
