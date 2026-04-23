// FICHIER : src-tauri/src/json_db/indexes/driver.rs

use super::{IndexDefinition, IndexRecord};
use crate::utils::prelude::*;

/// Trait définissant le comportement d'une structure d'index en mémoire
pub trait IndexMap: Default + Serializable + DeserializableOwned {
    fn insert_record(&mut self, key: String, doc_id: String);
    fn remove_record(&mut self, key: &str, doc_id: &str);
    fn get_doc_ids(&self, key: &str) -> Option<&Vec<String>>;
    fn from_records(records: Vec<IndexRecord>) -> Self;
    fn to_records(&self) -> Vec<IndexRecord>;
}

// --- Implémentation pour Hash Index (UnorderedMap) ---
impl IndexMap for UnorderedMap<String, Vec<String>> {
    fn insert_record(&mut self, key: String, doc_id: String) {
        self.entry(key).or_default().push(doc_id);
    }

    fn remove_record(&mut self, key: &str, doc_id: &str) {
        if let Some(ids) = self.get_mut(key) {
            ids.retain(|id| id != doc_id);
            if ids.is_empty() {
                self.remove(key);
            }
        }
    }

    fn get_doc_ids(&self, key: &str) -> Option<&Vec<String>> {
        self.get(key)
    }

    fn from_records(records: Vec<IndexRecord>) -> Self {
        let mut map: UnorderedMap<String, Vec<String>> = UnorderedMap::new();
        for r in records {
            map.entry(r.key).or_default().push(r.document_id);
        }
        map
    }

    fn to_records(&self) -> Vec<IndexRecord> {
        let mut records = Vec::new();
        for (k, ids) in self {
            for id in ids {
                records.push(IndexRecord {
                    key: k.clone(),
                    document_id: id.clone(),
                });
            }
        }
        records
    }
}

// --- Implémentation pour BTree Index (OrderedMap) ---
impl IndexMap for OrderedMap<String, Vec<String>> {
    fn insert_record(&mut self, key: String, doc_id: String) {
        self.entry(key).or_default().push(doc_id);
    }

    fn remove_record(&mut self, key: &str, doc_id: &str) {
        if let Some(ids) = self.get_mut(key) {
            ids.retain(|id| id != doc_id);
            if ids.is_empty() {
                self.remove(key);
            }
        }
    }

    fn get_doc_ids(&self, key: &str) -> Option<&Vec<String>> {
        self.get(key)
    }

    fn from_records(records: Vec<IndexRecord>) -> Self {
        let mut map: OrderedMap<String, Vec<String>> = OrderedMap::new();
        for r in records {
            map.entry(r.key).or_default().push(r.document_id);
        }
        map
    }

    fn to_records(&self) -> Vec<IndexRecord> {
        let mut records = Vec::new();
        for (k, ids) in self {
            for id in ids {
                records.push(IndexRecord {
                    key: k.clone(),
                    document_id: id.clone(),
                });
            }
        }
        records
    }
}

// --- Logique I/O Générique (Async) ---

pub async fn load<T: IndexMap>(path: &Path) -> RaiseResult<T> {
    if !fs::exists_async(path).await {
        return Ok(T::default());
    }
    let records: Vec<IndexRecord> = match fs::read_json_compressed_async(path).await {
        Ok(r) => r,
        Err(e) => raise_error!(
            "ERR_DB_INDEX_LOAD_FAILED",
            error = e,
            context = json_value!({ "path": path.to_string_lossy() })
        ),
    };
    Ok(T::from_records(records))
}

pub async fn save<T: IndexMap>(path: &Path, index: &T) -> RaiseResult<()> {
    let records = index.to_records();
    match fs::write_json_compressed_atomic_async(path, &records).await {
        Ok(_) => Ok(()),
        Err(e) => raise_error!(
            "ERR_DB_INDEX_SAVE_FAILED",
            error = e,
            context = json_value!({ "path": path.to_string_lossy() })
        ),
    }
}

pub async fn search<T: IndexMap>(path: &Path, key: &str) -> RaiseResult<Vec<String>> {
    let index: T = match load(path).await {
        Ok(idx) => idx,
        Err(e) => return Err(e),
    };
    Ok(index.get_doc_ids(key).cloned().unwrap_or_default())
}

pub async fn update<T: IndexMap>(
    path: &Path,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&JsonValue>,
    new_doc: Option<&JsonValue>,
) -> RaiseResult<()> {
    let mut index: T = load(path).await?;
    let mut changed = false;

    // Suppression
    if let Some(doc) = old_doc {
        if let Some(old_key) = doc.pointer(&def.field_path) {
            index.remove_record(&old_key.to_string(), doc_id);
            changed = true;
        }
    }

    // Ajout
    if let Some(doc) = new_doc {
        if let Some(new_key) = doc.pointer(&def.field_path) {
            let key_str = new_key.to_string();

            // Unicité
            if def.unique {
                if let Some(ids) = index.get_doc_ids(&key_str) {
                    if !ids.is_empty() && (ids.len() > 1 || ids[0] != doc_id) {
                        raise_error!(
                            "ERR_DB_INDEX_UNIQUE_VIOLATION",
                            error = format!(
                                "Violation de contrainte unique sur l'index '{}'",
                                def.name
                            ),
                            context = json_value!({
                                "index_name": def.name,
                                "conflicting_value": key_str,
                                "existing_ids": ids,
                                "target_doc_id": doc_id,
                                "action": "enforce_unique_constraint"
                            })
                        );
                    }
                }
            }

            index.insert_record(key_str, doc_id.to_string());
            changed = true;
        }
    }

    if changed {
        save(path, &index).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;

    #[test]
    fn test_driver_map_logic() {
        let mut map: UnorderedMap<String, Vec<String>> = UnorderedMap::new();
        map.insert_record("alice".into(), "1".into());
        map.insert_record("bob".into(), "2".into());
        map.insert_record("alice".into(), "3".into());

        assert_eq!(map.get_doc_ids("alice").unwrap().len(), 2);

        map.remove_record("alice", "1");
        assert_eq!(map.get_doc_ids("alice").unwrap().len(), 1);
        assert_eq!(map.get_doc_ids("alice").unwrap()[0], "3");
    }

    #[async_test]
    async fn test_driver_io_roundtrip_and_search() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.bin");

        // 1. Save (Async)
        let mut index: UnorderedMap<String, Vec<String>> = UnorderedMap::new();
        index.insert_record("key1".into(), "doc1".into());
        save(&path, &index).await.unwrap();

        // 2. Load (Async)
        let loaded: UnorderedMap<String, Vec<String>> = load(&path).await.unwrap();
        assert_eq!(loaded.get_doc_ids("key1").unwrap()[0], "doc1");

        // 3. Search (Async)
        let results = search::<UnorderedMap<String, Vec<String>>>(&path, "key1")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");

        let empty = search::<UnorderedMap<String, Vec<String>>>(&path, "missing")
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[async_test]
    async fn test_driver_update_logic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("update_test.bin");
        let def = IndexDefinition {
            name: "test".into(),
            field_path: "/val".into(),
            index_type: IndexType::Hash,
            unique: true,
        };

        let doc = json::json_value!({"val": "A"});

        // Initial update
        update::<UnorderedMap<String, Vec<String>>>(&path, &def, "id1", None, Some(&doc))
            .await
            .unwrap();

        let results = search::<UnorderedMap<String, Vec<String>>>(&path, "\"A\"")
            .await
            .unwrap();
        assert_eq!(results, vec!["id1"]);
    }
}
