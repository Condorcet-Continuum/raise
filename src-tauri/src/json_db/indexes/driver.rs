use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use super::{IndexDefinition, IndexRecord};
use crate::json_db::storage::file_storage::atomic_write_binary;

/// Trait définissant le comportement d'une structure d'index en mémoire
pub trait IndexMap: Default + Serialize + DeserializeOwned {
    fn insert_record(&mut self, key: String, doc_id: String);
    fn remove_record(&mut self, key: &str, doc_id: &str);
    fn get_doc_ids(&self, key: &str) -> Option<&Vec<String>>;
    fn from_records(records: Vec<IndexRecord>) -> Self;
    fn to_records(&self) -> Vec<IndexRecord>;
}

// --- Implémentation pour Hash Index (HashMap) ---
impl IndexMap for HashMap<String, Vec<String>> {
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
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for r in records {
            // CORRECTION : On utilise directement la clé (String) sans conversion
            map.entry(r.key).or_default().push(r.document_id);
        }
        map
    }

    fn to_records(&self) -> Vec<IndexRecord> {
        let mut records = Vec::new();
        for (k, ids) in self {
            for id in ids {
                // CORRECTION : On stocke directement la chaîne k (String)
                records.push(IndexRecord {
                    key: k.clone(),
                    document_id: id.clone(),
                });
            }
        }
        records
    }
}

// --- Implémentation pour BTree Index (BTreeMap) ---
impl IndexMap for BTreeMap<String, Vec<String>> {
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
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in records {
            // CORRECTION : Utilisation directe de la clé
            map.entry(r.key).or_default().push(r.document_id);
        }
        map
    }

    fn to_records(&self) -> Vec<IndexRecord> {
        let mut records = Vec::new();
        for (k, ids) in self {
            for id in ids {
                // CORRECTION : Utilisation directe de la clé
                records.push(IndexRecord {
                    key: k.clone(),
                    document_id: id.clone(),
                });
            }
        }
        records
    }
}

// --- Logique I/O Générique ---

/// Charge un index depuis le disque (Format Binaire Bincode)
pub fn load<T: IndexMap>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }

    let content = fs::read(path).with_context(|| format!("Lecture index {}", path.display()))?;

    // Désérialisation Bincode (Directe vers IndexRecord { key: String })
    let records: Vec<IndexRecord> = bincode::deserialize(&content)
        .with_context(|| format!("Désérialisation Bincode index {}", path.display()))?;

    Ok(T::from_records(records))
}

/// Sauvegarde un index sur le disque (Format Binaire Bincode)
pub fn save<T: IndexMap>(path: &Path, index: &T) -> Result<()> {
    let records = index.to_records();
    let encoded: Vec<u8> = bincode::serialize(&records)?;
    atomic_write_binary(path, &encoded)
}

/// Met à jour un index (charge -> modifie -> sauvegarde)
pub fn update<T: IndexMap>(
    path: &Path,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> Result<()> {
    let mut index: T = load(path)?;
    let mut changed = false;

    // 1. Suppression de l'ancienne entrée
    if let Some(doc) = old_doc {
        if let Some(old_key) = doc.pointer(&def.field_path) {
            // to_string() sur une Value donne sa représentation JSON (ex: "123" ou "\"abc\"")
            // C'est parfait pour une clé d'index générique.
            index.remove_record(&old_key.to_string(), doc_id);
            changed = true;
        }
    }

    // 2. Ajout de la nouvelle entrée
    if let Some(doc) = new_doc {
        if let Some(new_key) = doc.pointer(&def.field_path) {
            let key_str = new_key.to_string();

            // Vérification unicité
            if def.unique {
                if let Some(ids) = index.get_doc_ids(&key_str) {
                    if !ids.is_empty() && (ids.len() > 1 || ids[0] != doc_id) {
                        anyhow::bail!(
                            "Index unique constraint violation: {} = {}",
                            def.name,
                            key_str
                        );
                    }
                }
            }

            index.insert_record(key_str, doc_id.to_string());
            changed = true;
        }
    }

    if changed {
        save(path, &index)?;
    }

    Ok(())
}
