use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::json_db::storage::{file_storage, JsonDbConfig};

// Constantes partagées pour les tests
pub const TEST_SPACE: &str = "tests";
pub const TEST_DB: &str = "testU";

static ENV_INIT_LOCK: Mutex<()> = Mutex::new(());

pub struct TestEnv {
    pub cfg: JsonDbConfig,
    pub tmp_root: PathBuf,
    pub space: String,
    pub db: String,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.tmp_root);
    }
}

fn tmp_root() -> PathBuf {
    let base = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let thread_id = std::thread::current().id();
    let p = base.join(format!("jsondb_ut_{}_{:?}", ts, thread_id));
    fs::create_dir_all(&p).expect("création du domaine temporaire");
    p
}

fn find_repo_root(start: &Path) -> PathBuf {
    let mut cur = Some(start);
    while let Some(p) = cur {
        if p.join("schemas").join("v1").is_dir() {
            return p.to_path_buf();
        }
        cur = p.parent();
    }
    start.to_path_buf()
}

pub fn init_test_env() -> TestEnv {
    let _guard = ENV_INIT_LOCK.lock().unwrap();
    let tmp = tmp_root();

    // IMPORTANT: On set l'env var pour que JsonDbConfig::from_env la capte
    unsafe {
        std::env::set_var("PATH_GENAPTITUDE_DOMAIN", &tmp);
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = find_repo_root(&manifest);
    let cfg = JsonDbConfig::from_env(&repo_root).expect("JsonDbConfig::from_env");

    // On relâche le lock ici
    drop(_guard);

    TestEnv {
        cfg,
        tmp_root: tmp,
        space: TEST_SPACE.to_string(),
        db: TEST_DB.to_string(),
    }
}

/// Helper : S'assure que la DB existe physiquement (open ou create)
pub fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
    if file_storage::open_db(cfg, space, db).is_err() {
        file_storage::create_db(cfg, space, db).expect("create_db dans ensure_db_exists");
    }
}

/// Helper : Récupère la racine du dataset de test depuis l'ENV ou fallback
pub fn get_dataset_root() -> PathBuf {
    // Charge le .env si présent
    dotenvy::dotenv().ok();

    let path_str = std::env::var("PATH_GENAPTITUDE_DATASET").unwrap_or_else(|_| {
        // Fallback intelligent : on essaie de trouver le dossier examples
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = find_repo_root(&manifest);
        repo.join("examples/oa_miniproc/data")
            .to_string_lossy()
            .to_string()
    });

    let path_string = if path_str.starts_with("$HOME") {
        let home = std::env::var("HOME").expect("HOME non défini");
        path_str.replace("$HOME", &home)
    } else {
        path_str
    };

    PathBuf::from(path_string)
}
