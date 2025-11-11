use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

// ⬇️ Remplace "genaptitude" ci-dessous par le nom du package défini dans src-tauri/Cargo.toml si différent.
use genaptitude::json_db::storage::{
    file_storage::{create_db, drop_db, open_db, DropMode},
    JsonDbConfig,
};

fn make_temp_domain_root() -> PathBuf {
    let base = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let p = base.join(format!("jsondb_ut_{ts}"));
    fs::create_dir_all(&p).expect("mkdir temp domain root");
    p
}

fn find_repo_root(start: &Path) -> PathBuf {
    let mut cur: Option<&Path> = Some(start);
    while let Some(p) = cur {
        if p.join("schemas").join("v1").is_dir() {
            return p.to_path_buf();
        }
        cur = p.parent();
    }
    panic!(
        "schemas/v1 introuvable en remontant depuis {}",
        start.display()
    );
}

#[test]
fn db_lifecycle_minimal() {
    // 1) Domaine persistant isolé
    let tmp = make_temp_domain_root();
    std::env::set_var("PATH_GENAPTITUDE_DOMAIN", &tmp);

    // 2) Localise le repo root pour JsonDbConfig::from_env()
    // (tests d’intégration compilés dans src-tauri → CARGO_MANIFEST_DIR = src-tauri)
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = find_repo_root(&manifest);

    // 3) Construit la config
    let cfg = JsonDbConfig::from_env(&repo_root).expect("JsonDbConfig::from_env");

    // 4) Cycle minimal: create → open → drop (soft puis hard)
    let space = "un2";
    let db = "_system";

    // CREATE
    let handle = create_db(&cfg, space, db).expect("create_db");
    assert!(handle.root.is_dir(), "db root doit exister");
    let index_path = cfg.index_path(space, db);
    assert!(index_path.is_file(), "_system.json doit exister");

    // OPEN
    let opened = open_db(&cfg, space, db).expect("open_db");
    assert_eq!(opened.space, space);
    assert_eq!(opened.database, db);
    assert_eq!(opened.root, handle.root);

    // DROP (Soft) → renommage
    drop_db(&cfg, space, db, DropMode::Soft).expect("drop_db soft");
    assert!(
        !handle.root.exists(),
        "après soft drop, le dossier original ne doit plus exister"
    );
    // Vérifie qu’un dossier renommé `_system.deleted-<ts>` existe
    let mut found_soft = false;
    for entry in fs::read_dir(cfg.space_root(space)).expect("ls space_root") {
        let p = entry.expect("dirent").path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with(db) && name.contains(".deleted-") && p.is_dir() {
            found_soft = true;
            break;
        }
    }
    assert!(
        found_soft,
        "le dossier renommé *.deleted-<ts> doit exister (soft drop)"
    );

    // Re-crée puis DROP (Hard) → suppression définitive
    let handle2 = create_db(&cfg, space, db).expect("recreate_db");
    assert!(handle2.root.exists());
    drop_db(&cfg, space, db, DropMode::Hard).expect("drop_db hard");
    assert!(
        !cfg.db_root(space, db).exists(),
        "après hard drop, la DB doit être supprimée"
    );

    // Nettoyage best-effort
    let _ = fs::remove_dir_all(&tmp);
}
