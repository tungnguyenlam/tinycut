use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tinycut_core_domain::{
    DefaultModelSpec, DomainError, DomainResult, ModelInstallStatus, ModelManifest, ModelType,
    default_lightweight_models, default_model_manifest, default_model_spec,
};
use tokio::io::AsyncWriteExt;

/// Local model cache and downloader for TinyCut's default lightweight models.
///
/// Stores downloaded weights in `~/.tinycut/models` by default (overridable via
/// `TINYCUT_MODELS_DIR` or [`ModelManager::with_dir`]).
#[derive(Debug, Clone)]
pub struct ModelManager {
    models_dir: PathBuf,
    client: reqwest::Client,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::with_dir(Self::default_dir())
    }
}

impl ModelManager {
    /// Resolves the default on-disk model directory (`$TINYCUT_MODELS_DIR` or `~/.tinycut/models`).
    pub fn default_dir() -> PathBuf {
        if let Ok(custom) = std::env::var("TINYCUT_MODELS_DIR") {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".tinycut").join("models")
    }

    pub fn with_dir(models_dir: impl Into<PathBuf>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("tinycut/0.1")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            models_dir: models_dir.into(),
            client,
        }
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Expected local path for a given task's default lightweight model.
    pub fn model_path(&self, task: ModelType) -> Option<PathBuf> {
        default_model_spec(task).map(|spec| self.models_dir.join(spec.filename))
    }

    /// Returns `Some(path)` only if the model file is already present on disk and non-empty.
    pub fn resolve_if_installed(&self, task: ModelType) -> Option<PathBuf> {
        let path = self.model_path(task)?;
        let meta = std::fs::metadata(&path).ok()?;
        if meta.is_file() && meta.len() >= 1024 {
            Some(path)
        } else {
            None
        }
    }

    /// Returns the installation status of every default lightweight model in the catalog.
    pub fn list_statuses(&self) -> Vec<ModelInstallStatus> {
        default_lightweight_models()
            .into_iter()
            .map(|spec| self.status_for_spec(spec))
            .collect()
    }

    pub fn status_for(&self, task: ModelType) -> Option<ModelInstallStatus> {
        default_model_spec(task).map(|spec| self.status_for_spec(spec))
    }

    fn status_for_spec(&self, spec: DefaultModelSpec) -> ModelInstallStatus {
        let path = self.models_dir.join(&spec.filename);
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() && meta.len() >= 1024 => ModelInstallStatus {
                spec,
                installed: true,
                local_path: Some(path.display().to_string()),
                bytes_on_disk: meta.len(),
            },
            _ => ModelInstallStatus {
                spec,
                installed: false,
                local_path: None,
                bytes_on_disk: 0,
            },
        }
    }

    /// Returns the built-in manifest representing the default lightweight model catalog.
    pub fn default_manifest() -> ModelManifest {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        default_model_manifest(now)
    }

    /// Ensures the default lightweight model for `task` is downloaded locally.
    /// Downloads atomically to a `.part` file, verifies SHA-256 when pinned, and renames into place.
    pub async fn ensure_model(&self, task: ModelType) -> DomainResult<PathBuf> {
        if let Some(existing) = self.resolve_if_installed(task) {
            return Ok(existing);
        }
        let spec = default_model_spec(task).ok_or_else(|| {
            DomainError::Storage(format!("no default lightweight model registered for {task:?}"))
        })?;
        self.download_spec(&spec).await
    }

    /// Ensures all companion files for a task are installed (e.g. Qwen GGUF + its tokenizer JSON).
    pub async fn ensure_task_bundle(&self, task: ModelType) -> DomainResult<Vec<ModelInstallStatus>> {
        let _ = self.ensure_model(task).await?;
        if task == ModelType::TextFixer {
            let _ = self.ensure_model(ModelType::TextFixerTokenizer).await?;
        }
        if task == ModelType::ScreenOcrDet {
            let _ = self.ensure_model(ModelType::ScreenOcrRec).await?;
        }
        Ok(self.list_statuses())
    }

    /// Downloads every missing default lightweight model across all tasks.
    pub async fn ensure_all_defaults(&self) -> DomainResult<Vec<ModelInstallStatus>> {
        for task in ModelType::ALL_DOWNLOADABLE {
            self.ensure_model(task).await?;
        }
        Ok(self.list_statuses())
    }

    /// Removes a downloaded model file from the local cache.
    pub fn delete_model(&self, task: ModelType) -> DomainResult<()> {
        if let Some(path) = self.model_path(task)
            && path.exists()
        {
            std::fs::remove_file(&path).map_err(|e| {
                DomainError::Storage(format!(
                    "failed to remove model {}: {e}",
                    path.display()
                ))
            })?;
        }
        Ok(())
    }

    async fn download_spec(&self, spec: &DefaultModelSpec) -> DomainResult<PathBuf> {
        tokio::fs::create_dir_all(&self.models_dir)
            .await
            .map_err(|e| {
                DomainError::Storage(format!(
                    "failed to create models dir {}: {e}",
                    self.models_dir.display()
                ))
            })?;

        let final_path = self.models_dir.join(&spec.filename);
        let part_path = self.models_dir.join(format!("{}.part", spec.filename));

        tracing::info!(
            model_id = %spec.id,
            url = %spec.download_url,
            dest = %final_path.display(),
            "downloading default lightweight model"
        );

        let response = self
            .client
            .get(&spec.download_url)
            .send()
            .await
            .map_err(|e| {
                DomainError::Storage(format!("failed to request {}: {e}", spec.download_url))
            })?;

        if !response.status().is_success() {
            return Err(DomainError::Storage(format!(
                "model download {} returned HTTP {}",
                spec.id,
                response.status()
            )));
        }

        let mut file = tokio::fs::File::create(&part_path).await.map_err(|e| {
            DomainError::Storage(format!(
                "failed to create temp file {}: {e}",
                part_path.display()
            ))
        })?;

        let mut hasher = Sha256::new();
        let mut total_bytes: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| {
                DomainError::Storage(format!("download stream error for {}: {e}", spec.id))
            })?;
            hasher.update(&bytes);
            file.write_all(&bytes).await.map_err(|e| {
                DomainError::Storage(format!("failed writing {}: {e}", part_path.display()))
            })?;
            total_bytes += bytes.len() as u64;
        }
        file.flush()
            .await
            .map_err(|e| DomainError::Storage(format!("failed flushing model file: {e}")))?;
        drop(file);

        if total_bytes < 1024 {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(DomainError::Storage(format!(
                "downloaded file for {} is unexpectedly small ({total_bytes} bytes)",
                spec.id
            )));
        }

        if !spec.sha256.is_empty() {
            let digest = format!("{:x}", hasher.finalize());
            if !digest.eq_ignore_ascii_case(&spec.sha256) {
                let _ = tokio::fs::remove_file(&part_path).await;
                return Err(DomainError::Storage(format!(
                    "SHA-256 mismatch for {}: expected {}, got {}",
                    spec.id, spec.sha256, digest
                )));
            }
        }

        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(|e| {
                DomainError::Storage(format!(
                    "failed to move {} into {}: {e}",
                    part_path.display(),
                    final_path.display()
                ))
            })?;

        Ok(final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_covers_all_downloadable_tasks() {
        let dir = std::env::temp_dir().join(format!("tinycut_hub_test_{}", std::process::id()));
        let mgr = ModelManager::with_dir(&dir);
        let statuses = mgr.list_statuses();
        assert_eq!(statuses.len(), ModelType::ALL_DOWNLOADABLE.len());
        for st in &statuses {
            assert!(!st.spec.download_url.is_empty());
            assert!(
                st.spec.license == "MIT" || st.spec.license == "Apache-2.0",
                "model {} must use MIT or Apache-2.0",
                st.spec.id
            );
            assert!(!st.installed);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolves_installed_model_when_file_present() {
        let dir =
            std::env::temp_dir().join(format!("tinycut_hub_installed_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = ModelManager::with_dir(&dir);
        let target = mgr.model_path(ModelType::FaceDetector).unwrap();
        std::fs::write(&target, vec![0u8; 2048]).unwrap();

        assert_eq!(mgr.resolve_if_installed(ModelType::FaceDetector), Some(target));
        let st = mgr.status_for(ModelType::FaceDetector).unwrap();
        assert!(st.installed);
        assert_eq!(st.bytes_on_disk, 2048);

        mgr.delete_model(ModelType::FaceDetector).unwrap();
        assert!(mgr.resolve_if_installed(ModelType::FaceDetector).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
