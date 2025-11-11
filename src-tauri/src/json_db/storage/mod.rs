pub mod file_storage;

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct JsonDbConfig {
    pub domain_root: PathBuf,
    pub schemas_dev_root: PathBuf,
}

impl JsonDbConfig {
    pub fn from_env(repo_root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();
        let domain_root = std::env::var("PATH_GENAPTITUDE_DOMAIN")
            .map(PathBuf::from)
            .map_err(|e| anyhow::anyhow!("ENV PATH_GENAPTITUDE_DOMAIN manquant: {e}"))?;
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

// Stub temporaire attendu ailleurs; ne gêne pas les tests DB
#[allow(dead_code)]
pub struct StorageEngine;
