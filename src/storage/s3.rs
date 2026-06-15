use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::Result;
use crate::errors::AppError;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::primitives::ByteStream;
use serde::{Deserialize, Serialize};

use crate::models::AppConfig;
use crate::storage::Workspace;
use crate::storage::atomic::atomic_write;
use crate::storage::migration::migrate_config;
use crate::storage::validate_relative_content_path;

const BACKUP_MANIFEST_KEY: &str = ".gimji/backup-manifest.json";
const BACKUP_MANIFEST_VERSION: u32 = 1;

fn s3_service_error<E>(operation: &str, key: &str, source: &E) -> AppError
where
    E: ProvideErrorMetadata + std::fmt::Display,
{
    let detail = match (source.code(), source.message()) {
        (Some(code), Some(message)) => format!("{code}: {message}"),
        (Some(code), None) => code.to_owned(),
        (None, Some(message)) => message.to_owned(),
        (None, None) => source.to_string(),
    };

    AppError::S3ConnectionFailed(format!("{operation} failed for '{key}': {detail}"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct S3ConnectionSettings {
    pub endpoint_url: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
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
                .map_err(|source| s3_service_error("ListBuckets", "<buckets>", &source))?;
        } else {
            client
                .head_bucket()
                .bucket(bucket)
                .send()
                .await
                .map_err(|source| s3_service_error("HeadBucket", bucket, &source))?;
        }

        Ok(())
    }

    pub async fn backup_workspace(&self, workspace: &Workspace) -> Result<()> {
        self.validate_for_backup()?;

        let client = self.client();
        let bucket = self.bucket.trim();
        let files = workspace_backup_files(workspace.root())?;
        for file in &files {
            let bytes = fs::read(&file.path).map_err(|source| AppError::io(&file.path, source))?;
            let key = self.object_key(&file.key);
            client
                .put_object()
                .bucket(bucket)
                .key(&key)
                .body(ByteStream::from(bytes))
                .send()
                .await
                .map_err(|source| s3_service_error("PutObject", &key, &source))?;
        }

        let manifest = backup_manifest_for_files(&files)?;
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|source| AppError::json(BACKUP_MANIFEST_KEY, source))?;
        let key = self.object_key(BACKUP_MANIFEST_KEY);
        client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(ByteStream::from(manifest_bytes))
            .send()
            .await
            .map_err(|source| s3_service_error("PutObject", &key, &source))?;

        Ok(())
    }

    pub async fn restore_workspace(&self, workspace: &Workspace) -> Result<()> {
        self.validate_for_backup()?;

        let client = self.client();
        let bucket = self.bucket.trim();
        let root = workspace.root();

        let config_bytes = self
            .object_bytes(&client, &self.object_key("config.json"))
            .await?;
        let mut content_objects = Vec::new();

        let manifest = self.backup_manifest(&client, bucket).await?;
        let content_keys = match &manifest {
            Some(manifest) => content_keys_from_parsed_manifest(manifest)?,
            None => self.content_object_keys(&client, bucket).await?,
        };

        for key in content_keys {
            let bytes = self.object_bytes(&client, &self.object_key(&key)).await?;
            content_objects.push((key, bytes));
        }

        if let Some(manifest) = &manifest {
            validate_manifest_checksums(manifest, &config_bytes, &content_objects)?;
        }

        write_restore_objects(root, &config_bytes, &content_objects)?;

        Ok(())
    }

    pub async fn read_text_object(&self, key: &str) -> Result<String> {
        self.validate_for_backup()?;

        let bytes = self
            .object_bytes(&self.client(), &self.object_key(key))
            .await?;

        String::from_utf8(bytes).map_err(|source| AppError::S3ConnectionFailed(source.to_string()))
    }

    fn object_key(&self, key: &str) -> String {
        let prefix = normalize_prefix(&self.prefix);
        if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}/{key}")
        }
    }

    fn object_prefix(&self, prefix: &str) -> String {
        self.object_key(prefix)
    }

    fn local_object_key(&self, object_key: &str) -> Option<String> {
        let prefix = normalize_prefix(&self.prefix);
        if prefix.is_empty() {
            return Some(object_key.to_owned());
        }

        object_key
            .strip_prefix(&format!("{prefix}/"))
            .map(str::to_owned)
    }

    async fn object_bytes(&self, client: &Client, key: &str) -> Result<Vec<u8>> {
        let object = client
            .get_object()
            .bucket(self.bucket.trim())
            .key(key)
            .send()
            .await
            .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;

        object
            .body
            .collect()
            .await
            .map(|body| body.into_bytes().to_vec())
            .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))
    }

    async fn content_object_keys(&self, client: &Client, bucket: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation_token = None;

        loop {
            let mut request = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(self.object_prefix("content/"));
            if let Some(token) = continuation_token {
                request = request.continuation_token(token);
            }

            let response = request
                .send()
                .await
                .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;

            for object in response.contents() {
                let Some(key) = object.key() else {
                    continue;
                };
                let Some(key) = self.local_object_key(key) else {
                    continue;
                };
                if key == "content/" || key.ends_with('/') {
                    continue;
                }
                validate_relative_content_path(&key)?;
                keys.push(key);
            }

            continuation_token = response.next_continuation_token().map(str::to_owned);
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(keys)
    }

    async fn backup_manifest(
        &self,
        client: &Client,
        bucket: &str,
    ) -> Result<Option<BackupManifest>> {
        let manifest_key = self.object_key(BACKUP_MANIFEST_KEY);
        let response = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&manifest_key)
            .send()
            .await
            .map_err(|source| AppError::S3ConnectionFailed(source.to_string()))?;

        let manifest_exists = response
            .contents()
            .iter()
            .any(|object| object.key() == Some(manifest_key.as_str()));
        if !manifest_exists {
            return Ok(None);
        }

        let bytes = self.object_bytes(client, &manifest_key).await?;
        manifest_from_bytes(&bytes).map(Some)
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
            // S3-compatible services such as MinIO can reject optional checksum headers.
            .request_checksum_calculation(
                aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired,
            )
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

fn normalize_prefix(prefix: &str) -> String {
    prefix
        .trim()
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceBackupFile {
    path: PathBuf,
    key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BackupManifest {
    version: u32,
    gimji_version: String,
    created_at: String,
    objects: Vec<BackupManifestObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BackupManifestObject {
    key: String,
    checksum: String,
}

fn workspace_backup_files(root: &Path) -> Result<Vec<WorkspaceBackupFile>> {
    let mut files = vec![backup_file(root, &root.join("config.json"))?];
    collect_content_files(root, &root.join("content"), &mut files)?;
    Ok(files)
}

fn backup_manifest_for_files(files: &[WorkspaceBackupFile]) -> Result<BackupManifest> {
    let mut objects = Vec::new();
    for file in files {
        let bytes = fs::read(&file.path).map_err(|source| AppError::io(&file.path, source))?;
        objects.push(BackupManifestObject {
            key: file.key.clone(),
            checksum: checksum_hex(&bytes),
        });
    }

    Ok(BackupManifest {
        version: BACKUP_MANIFEST_VERSION,
        gimji_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: chrono::Utc::now().to_rfc3339(),
        objects,
    })
}

#[cfg(test)]
fn content_keys_from_manifest(bytes: &[u8]) -> Result<Vec<String>> {
    let manifest = manifest_from_bytes(bytes)?;
    content_keys_from_parsed_manifest(&manifest)
}

fn manifest_from_bytes(bytes: &[u8]) -> Result<BackupManifest> {
    let manifest: BackupManifest = serde_json::from_slice(bytes)
        .map_err(|source| AppError::json(BACKUP_MANIFEST_KEY, source))?;
    if manifest.version != BACKUP_MANIFEST_VERSION {
        return Err(AppError::UnsupportedVersion {
            kind: "backup manifest",
            version: manifest.version,
        });
    }
    Ok(manifest)
}

fn content_keys_from_parsed_manifest(manifest: &BackupManifest) -> Result<Vec<String>> {
    let mut keys = Vec::new();
    for object in &manifest.objects {
        if object.key == "config.json" {
            continue;
        }
        validate_relative_content_path(&object.key)?;
        keys.push(object.key.clone());
    }
    Ok(keys)
}

fn validate_manifest_checksums(
    manifest: &BackupManifest,
    config_bytes: &[u8],
    content_objects: &[(String, Vec<u8>)],
) -> Result<()> {
    for object in &manifest.objects {
        let actual_checksum = if object.key == "config.json" {
            Some(checksum_hex(config_bytes))
        } else {
            content_objects
                .iter()
                .find(|(key, _)| key == &object.key)
                .map(|(_, bytes)| checksum_hex(bytes))
        };

        let Some(actual_checksum) = actual_checksum else {
            return Err(AppError::InvalidPath(format!(
                "missing restored manifest object: {}",
                object.key
            )));
        };

        if actual_checksum != object.checksum {
            return Err(AppError::InvalidPath(format!(
                "backup manifest checksum mismatch: {}",
                object.key
            )));
        }
    }

    Ok(())
}

fn checksum_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
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

fn write_restore_object(root: &Path, key: &str, bytes: &[u8]) -> Result<()> {
    if key != "config.json" {
        validate_relative_content_path(key)?;
    }

    atomic_write(&root.join(key), bytes)
}

fn write_restore_objects(
    root: &Path,
    config_bytes: &[u8],
    content_objects: &[(String, Vec<u8>)],
) -> Result<()> {
    validate_restore_payload(config_bytes, content_objects)?;

    for (key, bytes) in content_objects {
        write_restore_object(root, key, bytes)?;
    }
    write_restore_object(root, "config.json", config_bytes)
}

fn validate_restore_payload(
    config_bytes: &[u8],
    content_objects: &[(String, Vec<u8>)],
) -> Result<()> {
    let mut content_keys = HashSet::new();
    for (key, _) in content_objects {
        validate_relative_content_path(key)?;
        content_keys.insert(key.as_str());
    }

    let mut config: AppConfig = serde_json::from_slice(config_bytes)
        .map_err(|source| AppError::json("config.json", source))?;
    migrate_config(&mut config)?;

    for note in &config.notes {
        for tab in &note.tabs {
            validate_relative_content_path(&tab.file_name)?;
            if !content_keys.contains(tab.file_name.as_str()) {
                return Err(AppError::InvalidPath(format!(
                    "missing restored content file: {}",
                    tab.file_name
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::models::{AppConfig, Note, Tab, TabType};

    use super::*;

    #[test]
    fn restore_objects_do_not_replace_config_when_content_write_fails() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let root = temp_dir.path();
        fs::write(root.join("config.json"), "old config").expect("write old config");
        let content_objects = vec![("../outside.md".to_owned(), b"new content".to_vec())];

        let error = write_restore_objects(root, b"new config", &content_objects).unwrap_err();

        assert!(error.to_string().contains("invalid content path"));
        assert_eq!(
            fs::read_to_string(root.join("config.json")).expect("read config"),
            "old config"
        );
    }

    #[test]
    fn restore_objects_reject_config_that_references_missing_content_file() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let root = temp_dir.path();
        fs::write(root.join("config.json"), "old config").expect("write old config");
        let config_bytes = config_bytes_for_content_file("content/missing.md");
        let content_objects = Vec::new();

        let error = write_restore_objects(root, &config_bytes, &content_objects).unwrap_err();

        assert!(error.to_string().contains("missing restored content file"));
        assert_eq!(
            fs::read_to_string(root.join("config.json")).expect("read config"),
            "old config"
        );
    }

    #[test]
    fn backup_manifest_lists_config_and_content_objects_with_checksums() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let root = temp_dir.path();
        fs::write(root.join("config.json"), "config body").expect("write config");
        fs::create_dir_all(root.join("content")).expect("create content dir");
        fs::write(root.join("content/project.md"), "project body").expect("write content");
        let files = workspace_backup_files(root).expect("backup files");

        let manifest = backup_manifest_for_files(&files).expect("manifest");

        assert_eq!(manifest.version, BACKUP_MANIFEST_VERSION);
        assert_eq!(manifest.objects.len(), 2);
        assert!(manifest.objects.iter().any(|object| {
            object.key == "config.json" && object.checksum == checksum_hex(b"config body")
        }));
        assert!(manifest.objects.iter().any(|object| {
            object.key == "content/project.md" && object.checksum == checksum_hex(b"project body")
        }));
    }

    #[test]
    fn backup_manifest_content_keys_are_read_for_restore() {
        let manifest = BackupManifest {
            version: BACKUP_MANIFEST_VERSION,
            gimji_version: "0.1.0".to_owned(),
            created_at: "2026-06-15T00:00:00Z".to_owned(),
            objects: vec![
                BackupManifestObject {
                    key: "config.json".to_owned(),
                    checksum: checksum_hex(b"config body"),
                },
                BackupManifestObject {
                    key: "content/project.md".to_owned(),
                    checksum: checksum_hex(b"project body"),
                },
            ],
        };
        let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest json");

        let content_keys = content_keys_from_manifest(&manifest_bytes).expect("content keys");

        assert_eq!(content_keys, vec!["content/project.md"]);
    }

    #[test]
    fn backup_manifest_rejects_content_checksum_mismatch() {
        let manifest = BackupManifest {
            version: BACKUP_MANIFEST_VERSION,
            gimji_version: "0.1.0".to_owned(),
            created_at: "2026-06-15T00:00:00Z".to_owned(),
            objects: vec![
                BackupManifestObject {
                    key: "config.json".to_owned(),
                    checksum: checksum_hex(&config_bytes_for_content_file("content/project.md")),
                },
                BackupManifestObject {
                    key: "content/project.md".to_owned(),
                    checksum: checksum_hex(b"expected body"),
                },
            ],
        };
        let config_bytes = config_bytes_for_content_file("content/project.md");
        let content_objects = vec![("content/project.md".to_owned(), b"actual body".to_vec())];

        let error =
            validate_manifest_checksums(&manifest, &config_bytes, &content_objects).unwrap_err();

        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn s3_object_keys_are_scoped_by_normalized_workspace_prefix() {
        let settings = S3ConnectionSettings {
            prefix: " /projects/gimji-main// ".to_owned(),
            ..Default::default()
        };

        assert_eq!(
            settings.object_key("config.json"),
            "projects/gimji-main/config.json"
        );
        assert_eq!(
            settings.object_key("content/project.md"),
            "projects/gimji-main/content/project.md"
        );
        assert_eq!(
            settings.object_prefix("content/"),
            "projects/gimji-main/content/"
        );
    }

    fn config_bytes_for_content_file(file_name: &str) -> Vec<u8> {
        let tab = Tab {
            id: "tab".to_owned(),
            title: "Markdown".to_owned(),
            tab_type: TabType::Markdown,
            file_name: file_name.to_owned(),
            created_at: "2026-06-15T00:00:00Z".to_owned(),
            updated_at: "2026-06-15T00:00:00Z".to_owned(),
        };
        let note = Note {
            id: "note".to_owned(),
            title: "Project".to_owned(),
            created_at: "2026-06-15T00:00:00Z".to_owned(),
            updated_at: "2026-06-15T00:00:00Z".to_owned(),
            tabs: vec![tab],
        };
        let config = AppConfig {
            version: crate::models::config::CONFIG_VERSION,
            selected_note_id: Some("note".to_owned()),
            selected_tab_id: Some("tab".to_owned()),
            notes: vec![note],
        };

        serde_json::to_vec(&config).expect("serialize config")
    }
}
