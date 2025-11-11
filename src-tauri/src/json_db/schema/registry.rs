use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::json_db::storage::JsonDbConfig;

/// Registre ultra-simple des schémas chargés depuis la DB:
/// - clé: URI logique "db://{space}/{db}/schemas/v1/<relpath>.json"
/// - valeur: document JSON du schéma
#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    base_prefix: String,
    root: PathBuf,
    by_uri: HashMap<String, Value>,
}

impl SchemaRegistry {
    /// Charge tous les fichiers sous `<DB>/schemas/v1`
    pub fn from_db(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<Self> {
        let root = cfg.db_schemas_root(space, db);
        let base_prefix = format!("db://{}/{}/schemas/v1/", space, db);
        let mut by_uri = HashMap::new();

        if !root.exists() {
            anyhow::bail!("Schemas root not found: {}", root.display());
        }

        // parcours récursif
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in
                fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    // on ne charge que .json
                    if let Some(ext) = path.extension() {
                        if ext == "json" {
                            let rel =
                                pathdiff::diff_paths(&path, &root).unwrap_or_else(|| path.clone());
                            let rel_str = rel.to_string_lossy().replace('\\', "/"); // windows-friendly

                            let uri = format!("{}{}", base_prefix, rel_str);
                            let data = fs::read_to_string(&path)
                                .with_context(|| format!("Lecture schéma {}", path.display()))?;
                            let json: Value = serde_json::from_str(&data)
                                .with_context(|| format!("Parse JSON schéma {}", path.display()))?;
                            by_uri.insert(uri, json);
                        }
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

    /// Construit une URI logique depuis un chemin relatif (ex: "actors/actor.schema.json")
    pub fn uri(&self, rel: &str) -> String {
        normalize_uri(&format!("{}{}", self.base_prefix, rel))
    }

    /// Récupère un document de schéma par URI **sans fragment**.
    pub fn get_by_uri(&self, uri: &str) -> Option<&Value> {
        // accepter une URI avec un éventuel fragment et l'ignorer
        let (p, _frag) = split_fragment(uri);
        self.by_uri.get(&normalize_uri(p))
    }

    /// Normalise une URI jointe à partir d'une base et d'un chemin relatif
    pub fn join(&self, base_uri: &str, relative: &str) -> Result<String> {
        if relative.starts_with("db://") {
            return Ok(relative.to_string());
        }
        // on coupe la base au dernier '/'
        let (base_dir, _) = base_uri.rsplit_once('/').unwrap_or((base_uri, ""));
        // normalisation simple des '..' et '.'
        let mut parts: Vec<&str> = base_dir.split('/').collect();
        for seg in relative.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    let _ = parts.pop();
                }
                other => parts.push(other),
            }
        }
        Ok(parts.join("/"))
    }

    /// Racine FS, utile pour debug
    pub fn fs_root(&self) -> &Path {
        &self.root
    }

    /// Préfixe logique "db://.../schemas/v1/"
    pub fn base(&self) -> &str {
        &self.base_prefix
    }
    pub fn resolve_ref(&self, base_uri: &str, r: &str) -> Result<(String, Value)> {
        let (path_part, fragment) = r.split_once('#').unwrap_or((r, ""));
        let target_uri = if path_part.is_empty() {
            base_uri.to_string()
        } else {
            self.join(base_uri, path_part)?
        };

        // get_by_uri -> Option<&Value> : on convertit en Result + on clone la Value
        let doc = self
            .get_by_uri(&target_uri)
            .ok_or(anyhow!("schema not found in registry: {}", target_uri))?;

        if fragment.is_empty() {
            return Ok((target_uri, doc.clone()));
        }

        // fragment "#/$defs/..." => JSON Pointer
        let pointer = if let Some(rest) = fragment.strip_prefix('/') {
            format!("/{}", rest)
        } else {
            format!("/{}", fragment.trim_start_matches('#'))
        };

        let node = doc.pointer(&pointer).cloned().ok_or(anyhow!(
            "pointer not found: {} in {}",
            pointer,
            target_uri
        ))?;

        Ok((target_uri, node))
    }
}

fn split_fragment(uri: &str) -> (&str, Option<&str>) {
    if let Some(idx) = uri.find('#') {
        (&uri[..idx], Some(&uri[idx..]))
    } else {
        (uri, None)
    }
}

/// Normalise `db://a/b/../c` -> `db://a/c`
fn normalize_uri(u: &str) -> String {
    if let Some(rest) = u.strip_prefix("db://") {
        let mut parts: Vec<&str> = rest.split('/').collect();
        let mut out: Vec<&str> = Vec::with_capacity(parts.len());
        for p in parts.drain(..) {
            match p {
                "" | "." => continue,
                ".." => {
                    out.pop();
                }
                _ => out.push(p),
            }
        }
        format!("db://{}", out.join("/"))
    } else {
        // pas une URI db:// → retourner tel quel
        u.to_string()
    }
}
