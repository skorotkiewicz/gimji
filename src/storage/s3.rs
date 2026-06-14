use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::Result;
use crate::errors::AppError;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;

use crate::storage::Workspace;

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

    pub async fn backup_workspace(&self, workspace: &Workspace) -> Result<()> {
        self.validate_for_backup()?;

        let client = self.client();
        let bucket = self.bucket.trim();
        for file in workspace_backup_files(workspace.root())? {
            let bytes = fs::read(&file.path).map_err(|source| AppError::io(&file.path, source))?;
            client
                .put_object()
                .bucket(bucket)
                .key(file.key)
                .body(ByteStream::from(bytes))
                .send()
                .await
                .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;
        }

        Ok(())
    }

    pub async fn read_text_object(&self, key: &str) -> Result<String> {
        self.validate_for_backup()?;

        let object = self
            .client()
            .get_object()
            .bucket(self.bucket.trim())
            .key(key)
            .send()
            .await
            .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;
        let bytes = object
            .body
            .collect()
            .await
            .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?
            .into_bytes()
            .to_vec();

        String::from_utf8(bytes).map_err(|source| AppError::S3ConnectionFailed(source.to_string()))
    }

    fn validate_for_backup(&self) -> Result<()> {
        self.validate_for_connection()?;
        require_field("bucket", &self.bucket)?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceBackupFile {
    path: PathBuf,
    key: String,
}

fn workspace_backup_files(root: &Path) -> Result<Vec<WorkspaceBackupFile>> {
    let mut files = vec![backup_file(root, &root.join("config.json"))?];
    collect_content_files(root, &root.join("content"), &mut files)?;
    Ok(files)
}

fn collect_content_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<WorkspaceBackupFile>,
) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|source| AppError::io(directory, source))? {
        let entry = entry.map_err(|source| AppError::io(directory, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| AppError::io(&path, source))?;

        if file_type.is_dir() {
            collect_content_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(backup_file(root, &path)?);
        }
    }

    Ok(())
}

fn backup_file(root: &Path, path: &Path) -> Result<WorkspaceBackupFile> {
    Ok(WorkspaceBackupFile {
        path: path.to_path_buf(),
        key: object_key(root, path)?,
    })
}

fn object_key(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|source| AppError::InvalidPath(source.to_string()))?;
    let mut parts = Vec::new();

    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => return Err(AppError::InvalidPath(relative.display().to_string())),
        }
    }

    Ok(parts.join("/"))
}
