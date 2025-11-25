pub mod file_storage;

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct JsonDbConfig {
    pub domain_root: PathBuf,
    pub schemas_dev_root: PathBuf,
}

impl JsonDbConfig {
    pub fn from_env(repo_root: impl AsRef<Path>) -> anyhow::Result<Self> {
        // 1. Charge le .env
        let _ = dotenvy::dotenv();

        // 2. Récupère la variable brute
        let domain_path_str = std::env::var("PATH_GENAPTITUDE_DOMAIN")
            .map_err(|e| anyhow::anyhow!("ENV PATH_GENAPTITUDE_DOMAIN manquant: {e}"))?;

        // 3. Expansion manuelle de $HOME
        let domain_root = expand_path(&domain_path_str);

        let schemas_dev_root = repo_root.as_ref().join("schemas").join("v1");

        Ok(Self {
            domain_root,
            schemas_dev_root,
        })
    }

    #[inline]
    pub fn space_root(&self, space: &str) -> PathBuf {
        self.domain_root.join(space)
    }
    #[inline]
    pub fn db_root(&self, space: &str, db: &str) -> PathBuf {
        self.space_root(space).join(db)
    }
    #[inline]
    pub fn index_path(&self, space: &str, db: &str) -> PathBuf {
        self.db_root(space, db).join("_system.json")
    }
    #[inline]
    pub fn db_schemas_root(&self, space: &str, db: &str) -> PathBuf {
        self.db_root(space, db).join("schemas").join("v1")
    }
}

/// Helper local pour remplacer $HOME ou ~ par le vrai chemin home
fn expand_path(path: &str) -> PathBuf {
    let mut p = path.to_string();

    // Si le chemin contient $HOME ou commence par ~
    if p.contains("$HOME") || p.starts_with("~/") {
        // On récupère le HOME du système
        if let Ok(home) = std::env::var("HOME") {
            p = p.replace("$HOME", &home);
            if p.starts_with("~/") {
                p = p.replacen("~", &home, 1);
            }
        }
    }

    // Si après tout ça on a encore un chemin relatif qui n'est pas absolu
    // on peut vouloir le rendre absolu par rapport au CWD, mais restons simples pour l'instant.
    PathBuf::from(p)
}

// Stub temporaire attendu ailleurs
#[allow(dead_code)]
pub struct StorageEngine;
