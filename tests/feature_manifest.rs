#[test]
fn s3_feature_controls_s3_dependencies() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("[features]"));
    assert!(manifest.contains("s3 = [\"dep:aws-sdk-s3\", \"dep:tokio\"]"));
    assert!(manifest.contains("aws-sdk-s3 = { version = \"1.136.0\", optional = true }"));
    assert!(manifest.contains(
        "tokio = { version = \"1.52.3\", features = [\"rt-multi-thread\"], optional = true }"
    ));
}
