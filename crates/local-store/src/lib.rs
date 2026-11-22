use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use tinycut_core_domain::{
    DomainError, DomainResult, EditDiff, GlossaryEntry, InferenceMode, MediaRef, Project,
};
use uuid::Uuid;

/// Local persistence contract for the creator's Personal Glossary.
pub trait GlossaryStore: Send + Sync {
    fn upsert_term(&mut self, spoken_variant: &str, preferred_term: &str) -> DomainResult<()>;
    fn list_preferred_terms(&self) -> DomainResult<Vec<String>>;
    fn all_entries(&self) -> DomainResult<Vec<GlossaryEntry>>;
}

/// Creator face embedding stored locally for the Visual Privacy Shield whitelist.
#[derive(Debug, Clone, PartialEq)]
pub struct WhitelistedFace {
    pub id: String,
    pub label: String,
    pub embedding: Vec<f32>,
}

/// Thread-safe SQLite-backed local store for Projects, Personal Glossary, Face Whitelist,
/// and the offline opt-in telemetry queue.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> DomainResult<Self> {
        let conn = Connection::open(path)
            .map_err(|e| DomainError::Storage(format!("failed to open sqlite db: {e}")))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> DomainResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DomainError::Storage(format!("failed to open in-memory sqlite: {e}")))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn lock_conn(&self) -> DomainResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| DomainError::Storage(format!("sqlite mutex poisoned: {e}")))
    }

    fn migrate(&self) -> DomainResult<()> {
        self.lock_conn()?
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS glossary_entries (
                    spoken_key TEXT PRIMARY KEY,
                    spoken_variant TEXT NOT NULL,
                    preferred_term TEXT NOT NULL,
                    hit_count INTEGER NOT NULL DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                );

                CREATE TABLE IF NOT EXISTS projects (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    media_id TEXT NOT NULL,
                    media_path TEXT NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    width INTEGER NOT NULL,
                    height INTEGER NOT NULL,
                    fps REAL NOT NULL,
                    is_nda_private INTEGER NOT NULL,
                    inference_mode TEXT NOT NULL,
                    locale TEXT NOT NULL,
                    content_domain TEXT,
                    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                );

                CREATE TABLE IF NOT EXISTS face_whitelist (
                    id TEXT PRIMARY KEY,
                    label TEXT NOT NULL,
                    embedding_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL DEFAULT (unixepoch())
                );

                CREATE TABLE IF NOT EXISTS telemetry_queue (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    payload_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL DEFAULT (unixepoch())
                );
                ",
            )
            .map_err(|e| DomainError::Storage(format!("sqlite migration failed: {e}")))?;
        Ok(())
    }

    /// Thread-safe upsert usable through shared `&self` (e.g., Tauri managed state).
    pub fn upsert_glossary_term(
        &self,
        spoken_variant: &str,
        preferred_term: &str,
    ) -> DomainResult<()> {
        let spoken = spoken_variant.trim();
        let preferred = preferred_term.trim();
        if spoken.is_empty() || preferred.is_empty() {
            return Err(DomainError::Storage(
                "glossary spoken_variant and preferred_term must be non-empty".into(),
            ));
        }
        let key = spoken.to_lowercase();
        self.lock_conn()?
            .execute(
                "INSERT INTO glossary_entries (spoken_key, spoken_variant, preferred_term, hit_count, updated_at)
                 VALUES (?1, ?2, ?3, 1, unixepoch())
                 ON CONFLICT(spoken_key) DO UPDATE SET
                     spoken_variant = excluded.spoken_variant,
                     preferred_term = excluded.preferred_term,
                     hit_count = glossary_entries.hit_count + 1,
                     updated_at = unixepoch()",
                params![key, spoken, preferred],
            )
            .map_err(|e| DomainError::Storage(format!("failed to upsert glossary term: {e}")))?;
        Ok(())
    }

    pub fn save_project(&self, project: &Project) -> DomainResult<()> {
        let mode = match project.inference_mode {
            InferenceMode::Local => "local",
            InferenceMode::CloudBoost => "cloud_boost",
        };
        self.lock_conn()?
            .execute(
                "INSERT INTO projects (
                    id, title, media_id, media_path, duration_ms, width, height, fps,
                    is_nda_private, inference_mode, locale, content_domain, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, unixepoch())
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    media_path = excluded.media_path,
                    duration_ms = excluded.duration_ms,
                    width = excluded.width,
                    height = excluded.height,
                    fps = excluded.fps,
                    is_nda_private = excluded.is_nda_private,
                    inference_mode = excluded.inference_mode,
                    locale = excluded.locale,
                    content_domain = excluded.content_domain,
                    updated_at = unixepoch()",
                params![
                    project.id.to_string(),
                    project.title,
                    project.media.id.to_string(),
                    project.media.path,
                    project.media.duration_ms as i64,
                    project.media.width as i64,
                    project.media.height as i64,
                    project.media.fps as f64,
                    if project.is_nda_private { 1 } else { 0 },
                    mode,
                    project.locale,
                    project.content_domain,
                ],
            )
            .map_err(|e| DomainError::Storage(format!("failed to save project: {e}")))?;
        Ok(())
    }

    pub fn list_projects(&self) -> DomainResult<Vec<Project>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, media_id, media_path, duration_ms, width, height, fps,
                        is_nda_private, inference_mode, locale, content_domain
                 FROM projects ORDER BY updated_at DESC",
            )
            .map_err(|e| DomainError::Storage(format!("failed to query projects: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let media_id_str: String = row.get(2)?;
                let mode_str: String = row.get(9)?;
                Ok(Project {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    title: row.get(1)?,
                    media: MediaRef {
                        id: Uuid::parse_str(&media_id_str).unwrap_or_else(|_| Uuid::new_v4()),
                        path: row.get(3)?,
                        duration_ms: row.get::<_, i64>(4)?.max(0) as u64,
                        width: row.get::<_, i64>(5)?.max(0) as u32,
                        height: row.get::<_, i64>(6)?.max(0) as u32,
                        fps: row.get::<_, f64>(7)? as f32,
                    },
                    is_nda_private: row.get::<_, i64>(8)? != 0,
                    inference_mode: if mode_str == "cloud_boost" {
                        InferenceMode::CloudBoost
                    } else {
                        InferenceMode::Local
                    },
                    locale: row.get(10)?,
                    content_domain: row.get(11)?,
                })
            })
            .map_err(|e| DomainError::Storage(format!("failed to map projects: {e}")))?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(
                row.map_err(|e| DomainError::Storage(format!("failed to read project row: {e}")))?,
            );
        }
        Ok(projects)
    }

    pub fn upsert_face_whitelist(&self, face: &WhitelistedFace) -> DomainResult<()> {
        let json = serde_json::to_string(&face.embedding)
            .map_err(|e| DomainError::Storage(format!("failed to serialize embedding: {e}")))?;
        self.lock_conn()?
            .execute(
                "INSERT INTO face_whitelist (id, label, embedding_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET label = excluded.label, embedding_json = excluded.embedding_json",
                params![face.id, face.label, json],
            )
            .map_err(|e| DomainError::Storage(format!("failed to upsert face whitelist: {e}")))?;
        Ok(())
    }

    pub fn list_face_whitelist(&self) -> DomainResult<Vec<WhitelistedFace>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT id, label, embedding_json FROM face_whitelist ORDER BY created_at ASC")
            .map_err(|e| DomainError::Storage(format!("failed to query face whitelist: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let label: String = row.get(1)?;
                let json: String = row.get(2)?;
                let embedding: Vec<f32> = serde_json::from_str(&json).unwrap_or_default();
                Ok(WhitelistedFace {
                    id,
                    label,
                    embedding,
                })
            })
            .map_err(|e| DomainError::Storage(format!("failed to map face whitelist: {e}")))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(
                row.map_err(|e| DomainError::Storage(format!("failed to read face row: {e}")))?,
            );
        }
        Ok(out)
    }

    pub fn enqueue_text_diff(&self, diff: &EditDiff) -> DomainResult<()> {
        let payload = serde_json::to_string(diff)
            .map_err(|e| DomainError::Storage(format!("failed to serialize edit diff: {e}")))?;
        self.lock_conn()?
            .execute(
                "INSERT INTO telemetry_queue (payload_json) VALUES (?1)",
                params![payload],
            )
            .map_err(|e| DomainError::Storage(format!("failed to enqueue telemetry: {e}")))?;
        Ok(())
    }

    pub fn pending_telemetry_count(&self) -> DomainResult<usize> {
        let count: i64 = self
            .lock_conn()?
            .query_row("SELECT COUNT(*) FROM telemetry_queue", [], |r| r.get(0))
            .map_err(|e| DomainError::Storage(format!("failed to count telemetry: {e}")))?;
        Ok(count.max(0) as usize)
    }
}

impl GlossaryStore for SqliteStore {
    fn upsert_term(&mut self, spoken_variant: &str, preferred_term: &str) -> DomainResult<()> {
        self.upsert_glossary_term(spoken_variant, preferred_term)
    }

    fn list_preferred_terms(&self) -> DomainResult<Vec<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT preferred_term FROM glossary_entries ORDER BY preferred_term ASC")
            .map_err(|e| DomainError::Storage(format!("failed to query preferred terms: {e}")))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| DomainError::Storage(format!("failed to map preferred terms: {e}")))?;

        let mut terms = Vec::new();
        for row in rows {
            terms.push(
                row.map_err(|e| DomainError::Storage(format!("failed to read term: {e}")))?,
            );
        }
        Ok(terms)
    }

    fn all_entries(&self) -> DomainResult<Vec<GlossaryEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT spoken_variant, preferred_term, hit_count
                 FROM glossary_entries ORDER BY hit_count DESC, preferred_term ASC",
            )
            .map_err(|e| DomainError::Storage(format!("failed to query glossary entries: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(GlossaryEntry {
                    spoken_variant: r.get(0)?,
                    preferred_term: r.get(1)?,
                    hit_count: r.get::<_, i64>(2)?.max(0) as u32,
                })
            })
            .map_err(|e| DomainError::Storage(format!("failed to map glossary entries: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(
                row.map_err(|e| DomainError::Storage(format!("failed to read entry: {e}")))?,
            );
        }
        Ok(entries)
    }
}

/// In-memory store for lightweight tests and prototyping.
#[derive(Debug, Default)]
pub struct InMemoryGlossaryStore {
    entries: HashMap<String, GlossaryEntry>,
}

impl GlossaryStore for InMemoryGlossaryStore {
    fn upsert_term(&mut self, spoken_variant: &str, preferred_term: &str) -> DomainResult<()> {
        let key = spoken_variant.trim().to_lowercase();
        let entry = self.entries.entry(key).or_insert_with(|| GlossaryEntry {
            spoken_variant: spoken_variant.trim().to_string(),
            preferred_term: preferred_term.trim().to_string(),
            hit_count: 0,
        });
        entry.preferred_term = preferred_term.trim().to_string();
        entry.hit_count += 1;
        Ok(())
    }

    fn list_preferred_terms(&self) -> DomainResult<Vec<String>> {
        let mut terms: Vec<String> = self
            .entries
            .values()
            .map(|e| e.preferred_term.clone())
            .collect();
        terms.sort();
        terms.dedup();
        Ok(terms)
    }

    fn all_entries(&self) -> DomainResult<Vec<GlossaryEntry>> {
        Ok(self.entries.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycut_core_domain::EditType;

    #[test]
    fn glossary_tracks_and_deduplicates_terms() {
        let mut store = InMemoryGlossaryStore::default();
        store.upsert_term("cơ sơ", "Cursor").unwrap();
        store.upsert_term("mác búc", "MacBook").unwrap();
        store.upsert_term("cơ sơ", "Cursor").unwrap();

        let terms = store.list_preferred_terms().unwrap();
        assert_eq!(terms, vec!["Cursor", "MacBook"]);
    }

    #[test]
    fn sqlite_store_persists_glossary_projects_faces_and_telemetry() {
        let mut store = SqliteStore::open_in_memory().unwrap();
        store.upsert_term("cơ sơ", "Cursor").unwrap();
        store.upsert_term("cơ sơ", "Cursor").unwrap();
        store.upsert_term("mác búc", "MacBook").unwrap();

        assert_eq!(
            store.list_preferred_terms().unwrap(),
            vec!["Cursor", "MacBook"]
        );
        let entries = store.all_entries().unwrap();
        assert_eq!(entries[0].preferred_term, "Cursor");
        assert_eq!(entries[0].hit_count, 2);

        let project = Project {
            id: Uuid::new_v4(),
            title: "Street Vlog #1".into(),
            media: MediaRef {
                id: Uuid::new_v4(),
                path: "/videos/vlog1.mp4".into(),
                duration_ms: 60_000,
                width: 3840,
                height: 2160,
                fps: 60.0,
            },
            is_nda_private: true,
            inference_mode: InferenceMode::Local,
            locale: "vi-VN".into(),
            content_domain: Some("vlog".into()),
        };
        store.save_project(&project).unwrap();
        let listed = store.list_projects().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Street Vlog #1");
        assert!(listed[0].is_nda_private);

        store
            .upsert_face_whitelist(&WhitelistedFace {
                id: "creator-1".into(),
                label: "Host".into(),
                embedding: vec![0.6, 0.8],
            })
            .unwrap();
        assert_eq!(store.list_face_whitelist().unwrap().len(), 1);

        store
            .enqueue_text_diff(&EditDiff {
                raw_asr: "cơ sơ".into(),
                llm_draft: None,
                user_final: "Cursor".into(),
                edit_type: EditType::WordFix,
                audio_clip_ref: None,
            })
            .unwrap();
        assert_eq!(store.pending_telemetry_count().unwrap(), 1);
    }
}
