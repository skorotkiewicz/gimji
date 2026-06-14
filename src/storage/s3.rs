use crate::Result;
use crate::errors::AppError;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct S3ConnectionSettings {
    pub endpoint_url: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl S3ConnectionSettings {
    pub fn validate_for_connection(&self) -> Result<()> {
        require_field("endpoint URL", &self.endpoint_url)?;
        require_field("region", &self.region)?;
        require_field("access key ID", &self.access_key_id)?;
        require_field("secret access key", &self.secret_access_key)?;
        Ok(())
    }

    pub async fn test_connection(&self) -> Result<()> {
        self.validate_for_connection()?;

        let client = self.client();
        let bucket = self.bucket.trim();
        if bucket.is_empty() {
            client
                .list_buckets()
                .send()
                .await
                .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;
        } else {
            client
                .head_bucket()
                .bucket(bucket)
                .send()
                .await
                .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;
        }

        Ok(())
    }

    fn client(&self) -> Client {
        let credentials = Credentials::new(
            self.access_key_id.trim().to_owned(),
            self.secret_access_key.trim().to_owned(),
            None,
            None,
            "gimji",
        );
        let config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .endpoint_url(self.endpoint_url.trim())
            .force_path_style(true)
            .region(Region::new(self.region.trim().to_owned()))
            .build();

        Client::from_conf(config)
    }
}

fn require_field(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(AppError::InvalidS3Connection(format!(
            "{label} is required"
        )));
    }

    Ok(())
}
