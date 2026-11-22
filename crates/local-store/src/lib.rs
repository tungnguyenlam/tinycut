use std::collections::HashMap;
use tinycut_core_domain::{DomainResult, GlossaryEntry};

/// Local persistence contract (SQLite via `rusqlite`/`sqlx` in Phase 1).
pub trait GlossaryStore: Send + Sync {
    fn upsert_term(&mut self, spoken_variant: &str, preferred_term: &str) -> DomainResult<()>;
    fn list_preferred_terms(&self) -> DomainResult<Vec<String>>;
    fn all_entries(&self) -> DomainResult<Vec<GlossaryEntry>>;
}

/// In-memory store for tests and early prototyping.
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

    #[test]
    fn glossary_tracks_and_deduplicates_terms() {
        let mut store = InMemoryGlossaryStore::default();
        store.upsert_term("cơ sơ", "Cursor").unwrap();
        store.upsert_term("mác búc", "MacBook").unwrap();
        store.upsert_term("cơ sơ", "Cursor").unwrap();

        let terms = store.list_preferred_terms().unwrap();
        assert_eq!(terms, vec!["Cursor", "MacBook"]);
    }
}
