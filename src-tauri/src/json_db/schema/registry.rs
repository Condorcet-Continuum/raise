// FICHIER : src-tauri/src/json_db/schema/registry.rs

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::json_db::storage::JsonDbConfig;

/// Registre des schémas chargés depuis la DB.
#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    pub base_prefix: String,
    pub root: PathBuf,
    pub by_uri: HashMap<String, Value>,
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            base_prefix: String::new(),
            root: PathBuf::new(),
            by_uri: HashMap::new(),
        }
    }

    pub fn add(&mut self, uri: &str, schema: Value) {
        self.by_uri.insert(uri.to_string(), schema);
    }

    pub fn from_db(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<Self> {
        // CORRECTION : Utilisation de db_schemas_root pour inclure le dossier _system
        let root = cfg.db_schemas_root(space, db).join("v1");
        let base_prefix = format!("db://{}/{}/schemas/v1/", space, db);
        let mut by_uri = HashMap::new();

        if !root.exists() {
            // En production ou test vide, ce n'est pas une erreur, juste un registre vide
            return Ok(Self {
                base_prefix,
                root,
                by_uri,
            });
        }

        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in
                fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "json") {
                    if let Ok(rel) = path.strip_prefix(&root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let uri = format!("{}{}", base_prefix, rel_str);

                        let content = fs::read_to_string(&path)?;
                        let json: Value = serde_json::from_str(&content)
                            .with_context(|| format!("Json error in {}", path.display()))?;

                        by_uri.insert(uri, json);
                    }
                }
            }
        }

        Ok(Self {
            base_prefix,
            root,
            by_uri,
        })
    }

    pub fn get_by_uri(&self, uri: &str) -> Option<&Value> {
        self.by_uri.get(uri)
    }

    pub fn uri(&self, rel_path: &str) -> String {
        format!("{}{}", self.base_prefix, rel_path)
    }

    pub fn join(&self, base_uri: &str, relative_path: &str) -> Result<String> {
        if relative_path.contains("://") {
            return Ok(relative_path.to_string());
        }

        if base_uri.starts_with("db://") {
            let parent = if base_uri.ends_with('/') {
                base_uri.to_string()
            } else {
                let parts: Vec<&str> = base_uri.split('/').collect();
                parts[..parts.len() - 1].join("/") + "/"
            };

            // Simplification : concaténation directe.
            // Une vraie résolution de path ".." serait idéale mais complexe sans crate `url`
            let combined = format!("{}{}", parent, relative_path);
            Ok(combined)
        } else {
            Ok(relative_path.to_string())
        }
    }

    pub fn resolve_ref(&self, base_uri: &str, ref_val: &str) -> Result<(String, Value)> {
        let target_uri_full = self.join(base_uri, ref_val)?;
        let (file_uri, fragment) = split_fragment(&target_uri_full);

        let doc = self
            .get_by_uri(file_uri)
            .or_else(|| {
                // Fallback fuzzy search pour les cas complexes de ".."
                self.by_uri
                    .keys()
                    .find(|k| k.ends_with(file_uri.split('/').last().unwrap_or("")))
                    .and_then(|k| self.by_uri.get(k))
            })
            .ok_or_else(|| anyhow!("Schema not found in registry: {}", file_uri))?;

        if let Some(frag) = fragment {
            if frag.is_empty() {
                return Ok((file_uri.to_string(), doc.clone()));
            }

            let pointer = if frag.starts_with("#/") {
                &frag[1..]
            } else if frag.starts_with('#') {
                frag
            } else {
                frag
            };

            let node = doc
                .pointer(pointer)
                .cloned()
                .ok_or_else(|| anyhow!("Pointer not found: {} in {}", pointer, file_uri))?;
            return Ok((file_uri.to_string(), node));
        }

        Ok((file_uri.to_string(), doc.clone()))
    }

    pub fn clear(&mut self) {
        self.by_uri.clear();
    }
}

fn split_fragment(uri: &str) -> (&str, Option<&str>) {
    if let Some(idx) = uri.find('#') {
        (&uri[..idx], Some(&uri[idx..]))
    } else {
        (uri, None)
    }
}
