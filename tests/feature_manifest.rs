#[test]
fn s3_feature_controls_s3_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let rust_s3_dependency = concat!(
        "rust-s3 = { version = \"0.37.2\", default-features = false, ",
        "features = [\"fail-on-err\", \"tokio-rustls-tls\"], optional = true }"
    );

    assert!(manifest.contains("[features]"));
    assert!(manifest.contains("s3 = [\"dep:rust-s3\", \"dep:tokio\"]"));
    assert!(manifest.contains(rust_s3_dependency));
    assert!(manifest.contains(
        "tokio = { version = \"1.52.3\", features = [\"rt-multi-thread\"], optional = true }"
    ));
}
