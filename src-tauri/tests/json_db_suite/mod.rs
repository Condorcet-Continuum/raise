// FICHIER : src-tauri/tests/json_db_suite/mod.rs

use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

static INIT: Once = Once::new();

// On utilise les noms par défaut du système pour faciliter le chargement des schémas
pub const TEST_SPACE: &str = "un2";
pub const TEST_DB: &str = "_system";

pub struct TestEnv {
    pub cfg: JsonDbConfig,
    pub storage: StorageEngine,
    // On garde ces champs pour la compatibilité avec les tests existants
    pub space: String,
    pub db: String,
    // Le dossier temporaire est supprimé quand cette variable sort du scope
    pub _tmp_dir: tempfile::TempDir,
}

pub fn init_test_env() -> TestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let data_root = tmp_dir.path().to_path_buf();

    let cfg = JsonDbConfig {
        data_root: data_root.clone(),
    };

    // 1. CRÉATION DE L'ARBORESCENCE DB
    let db_root = cfg.db_root(TEST_SPACE, TEST_DB);
    fs::create_dir_all(&db_root).expect("create db root");

    // 2. LOCALISATION DES SCHÉMAS SOURCES
    // CARGO_MANIFEST_DIR pointe sur src-tauri
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // On remonte d'un cran pour trouver 'schemas/v1' à la racine du monorepo
    let schema_source = manifest_dir
        .parent() // remonte à la racine 'genaptitude'
        .unwrap()
        .join("schemas/v1");

    // 3. DESTINATION DES SCHÉMAS (Dans le dossier temporaire)
    // C'est ici que le SchemaRegistry va regarder
    let schemas_dest = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");

    if schema_source.exists() {
        if !schemas_dest.exists() {
            fs::create_dir_all(&schemas_dest).expect("create schemas dest dir");
        }
        copy_dir_recursive(&schema_source, &schemas_dest).expect("copy schemas");
    } else {
        eprintln!(
            "⚠️ CRITIQUE : Impossible de trouver les schémas sources dans {:?}",
            schema_source
        );
    }

    // 4. CRÉATION DE _system.json
    // Indispensable pour que le système considère la base comme valide
    let system_index_path = db_root.join("_system.json");
    if !system_index_path.exists() {
        let index_content = serde_json::json!({
            "space": TEST_SPACE,
            "database": TEST_DB,
            "collections": {}
        });
        fs::write(&system_index_path, index_content.to_string()).ok();
    }

    // 5. PRÉPARATION DU DATASET (Pour dataset_integration.rs)
    // On crée le fichier mock attendu par le test
    let dataset_dir = data_root.join("dataset/arcadia/v1/data/exchange-items");
    fs::create_dir_all(&dataset_dir).unwrap();
    fs::write(
        dataset_dir.join("position_gps.json"),
        r#"{ "name": "GPS Position", "exchangeMechanism": "Flow" }"#,
    )
    .unwrap();

    // On crée aussi le mock pour les articles (utilisé par json_db_integration)
    let article_dir = data_root.join("dataset/arcadia/v1/data/articles");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(
        article_dir.join("article.json"),
        r#"{ "handle": "test", "displayName": "Test", "slug": "test", "title": "Test", "status": "draft" }"#
    ).unwrap();

    let storage = StorageEngine::new(cfg.clone());

    TestEnv {
        cfg,
        storage,
        space: TEST_SPACE.to_string(),
        db: TEST_DB.to_string(),
        _tmp_dir: tmp_dir,
    }
}

// Helpers réutilisés par les tests
pub fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
    let p = cfg.db_root(space, db);
    if !p.exists() {
        fs::create_dir_all(p).unwrap();
    }
}

// Helper de copie récursive
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

// Helper pour récupérer un fichier du dataset (avec création parent si manquant)
pub fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
    let root = cfg.data_root.join("dataset");
    let path = root.join(rel_path);
    if let Some(p) = path.parent() {
        if !p.exists() {
            fs::create_dir_all(p).unwrap();
        }
    }
    path
}
