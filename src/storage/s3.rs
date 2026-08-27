use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::Result;
use crate::errors::AppError;

use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use serde::{Deserialize, Serialize};

use crate::models::AppConfig;
use crate::storage::Workspace;
use crate::storage::atomic::{atomic_write, atomic_write_private};
use crate::storage::validate_relative_content_path;

const BACKUP_MANIFEST_KEY: &str = ".gimji/backup-manifest.json";
const BACKUP_MANIFEST_VERSION: u32 = 1;
const LOCAL_SETTINGS_PATH: &str = ".app/s3.json";

fn s3_service_error(operation: &str, key: &str, source: impl std::fmt::Display) -> AppError {
    AppError::S3ConnectionFailed(format!("{operation} failed for '{key}': {source}"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct S3ConnectionSettings {
    pub endpoint_url: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl S3ConnectionSettings {
    pub fn load_local(workspace: &Workspace) -> Result<Option<Self>> {
        let path = workspace.root().join(LOCAL_SETTINGS_PATH);
        if !path.exists() {
            return Ok(None);
        }

        let text = fs::read_to_string(&path).map_err(|source| AppError::io(&path, source))?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|source| AppError::json(&path, source))
    }

    pub fn save_local(&self, workspace: &Workspace) -> Result<()> {
        let path = workspace.root().join(LOCAL_SETTINGS_PATH);
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|source| AppError::json(&path, source))?;
        atomic_write_private(&path, &bytes)
    }

    pub fn validate_for_connection(&self) -> Result<()> {
        require_field("endpoint URL", &self.endpoint_url)?;
        require_field("region", &self.region)?;
        require_field("access key ID", &self.access_key_id)?;
        require_field("secret access key", &self.secret_access_key)?;
        Ok(())
    }

    pub async fn test_connection(&self) -> Result<()> {
        self.validate_for_connection()?;

        let bucket = self.bucket.trim();
        if bucket.is_empty() {
            Bucket::list_buckets(self.region(), self.credentials()?)
                .await
                .map_err(|source| s3_service_error("ListBuckets", "<buckets>", source))?;
        } else {
            self.storage_bucket()?
                .list_page(String::new(), None, None, None, Some(1))
                .await
                .map_err(|source| s3_service_error("ListObjectsV2", bucket, source))?;
        }

        Ok(())
    }

    pub async fn backup_workspace(&self, workspace: &Workspace) -> Result<()> {
        self.validate_for_backup()?;

        let bucket = self.storage_bucket()?;
        let files = workspace_backup_files(workspace.root())?;
        for file in &files {
            let bytes = fs::read(&file.path).map_err(|source| AppError::io(&file.path, source))?;
            let key = self.object_key(&file.key);
            bucket
                .put_object(&key, &bytes)
                .await
                .map_err(|source| s3_service_error("PutObject", &key, source))?;
        }

        let manifest = backup_manifest_for_files(&files)?;
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|source| AppError::json(BACKUP_MANIFEST_KEY, source))?;
        let key = self.object_key(BACKUP_MANIFEST_KEY);
        bucket
            .put_object(&key, &manifest_bytes)
            .await
            .map_err(|source| s3_service_error("PutObject", &key, source))?;

        Ok(())
    }

    pub async fn restore_workspace(&self, workspace: &Workspace) -> Result<()> {
        self.validate_for_backup()?;

        let bucket = self.storage_bucket()?;
        let root = workspace.root();

        let config_bytes = self
            .object_bytes(&bucket, &self.object_key("config.json"))
            .await?;
        let mut content_objects = Vec::new();

        let manifest = self.backup_manifest(&bucket).await?;
        let content_keys = match &manifest {
            Some(manifest) => content_keys_from_parsed_manifest(manifest)?,
            None => self.content_object_keys(&bucket).await?,
        };

        for key in content_keys {
            let bytes = self.object_bytes(&bucket, &self.object_key(&key)).await?;
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

        let bucket = self.storage_bucket()?;
        let bytes = self.object_bytes(&bucket, &self.object_key(key)).await?;

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

    async fn object_bytes(&self, bucket: &Bucket, key: &str) -> Result<Vec<u8>> {
        let object = bucket
            .get_object(key)
            .await
            .map_err(|source| s3_service_error("GetObject", key, source))?;

        Ok(object.as_slice().to_vec())
    }

    async fn content_object_keys(&self, bucket: &Bucket) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let results = bucket
            .list(self.object_prefix("content/"), None)
            .await
            .map_err(|source| s3_service_error("ListObjectsV2", "content/", source))?;

        for result in results {
            for object in result.contents {
                let Some(key) = self.local_object_key(&object.key) else {
                    continue;
                };
                if key == "content/" || key.ends_with('/') {
                    continue;
                }
                validate_relative_content_path(&key)?;
                keys.push(key);
            }
        }

        Ok(keys)
    }

    async fn backup_manifest(&self, bucket: &Bucket) -> Result<Option<BackupManifest>> {
        let manifest_key = self.object_key(BACKUP_MANIFEST_KEY);
        let manifest_exists = bucket
            .object_exists(&manifest_key)
            .await
            .map_err(|source| s3_service_error("HeadObject", &manifest_key, source))?;
        if !manifest_exists {
            return Ok(None);
        }

        let bytes = self.object_bytes(bucket, &manifest_key).await?;
        manifest_from_bytes(&bytes).map(Some)
    }

    fn validate_for_backup(&self) -> Result<()> {
        self.validate_for_connection()?;
        require_field("bucket", &self.bucket)?;
        Ok(())
    }

    fn storage_bucket(&self) -> Result<Box<Bucket>> {
        Bucket::new(self.bucket.trim(), self.region(), self.credentials()?)
            .map(|bucket| bucket.with_path_style())
            .map_err(|source| s3_service_error("CreateBucketClient", self.bucket.trim(), source))
    }

    fn credentials(&self) -> Result<Credentials> {
        Credentials::new(
            Some(self.access_key_id.trim()),
            Some(self.secret_access_key.trim()),
            None,
            None,
            None,
        )
        .map_err(|source| AppError::InvalidS3Connection(source.to_string()))
    }

    fn region(&self) -> Region {
        Region::Custom {
            region: self.region.trim().to_owned(),
            endpoint: self.endpoint_url.trim().to_owned(),
        }
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

    let config: AppConfig = serde_json::from_slice(config_bytes)
        .map_err(|source| AppError::json("config.json", source))?;

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
        fs::create_dir_all(root.join(".app")).expect("create local app dir");
        fs::write(root.join(LOCAL_SETTINGS_PATH), "local secret").expect("write local settings");
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
    fn local_settings_round_trip_with_credentials() {
        let temp_dir = tempfile::tempdir().expect("temporary workspace");
        let workspace = Workspace::create(temp_dir.path()).expect("workspace");
        let settings = S3ConnectionSettings {
            endpoint_url: "http://localhost:9000".to_owned(),
            region: "us-east-1".to_owned(),
            bucket: "storage".to_owned(),
            prefix: "gimji".to_owned(),
            access_key_id: "access".to_owned(),
            secret_access_key: "secret".to_owned(),
        };

        settings
            .save_local(&workspace)
            .expect("save local settings");

        assert_eq!(
            S3ConnectionSettings::load_local(&workspace).expect("load local settings"),
            Some(settings)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(workspace.root().join(LOCAL_SETTINGS_PATH))
                .expect("settings metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
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
            selected_note_id: Some("note".to_owned()),
            selected_tab_id: Some("tab".to_owned()),
            notes: vec![note],
        };

        serde_json::to_vec(&config).expect("serialize config")
    }
}
