use std::{
    fs,
    path::Path,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use genaptitude::json_db::storage::{
    file_storage::{create_db, drop_db, open_db, DropMode},
    JsonDbConfig,
};

fn tmp_root() -> PathBuf {
    let base = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let p = base.join(format!("jsondb_ut_idemp_{ts}"));
    fs::create_dir_all(&p).unwrap();
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
    panic!("schemas/v1 introuvable depuis {}", start.display());
}

#[test]
fn drop_is_idempotent_and_recreate_works() {
    let tmp = tmp_root();
    std::env::set_var("PATH_GENAPTITUDE_DOMAIN", &tmp);

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = find_repo_root(&manifest);
    let cfg = JsonDbConfig::from_env(&repo_root).expect("cfg");

    let (space, db) = ("un2", "_system");

    // 1) drop sur DB inexistante → OK (idempotent)
    drop_db(&cfg, space, db, DropMode::Soft).expect("soft drop inexistant");
    drop_db(&cfg, space, db, DropMode::Hard).expect("hard drop inexistant");

    // 2) create → open → hard drop
    let h = create_db(&cfg, space, db).expect("create");
    assert!(h.root.exists());
    let _ = open_db(&cfg, space, db).expect("open");
    drop_db(&cfg, space, db, DropMode::Hard).expect("hard drop");

    assert!(!cfg.db_root(space, db).exists());
    let _ = fs::remove_dir_all(tmp);
}
