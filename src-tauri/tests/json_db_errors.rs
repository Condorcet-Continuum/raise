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
    let p = base.join(format!("jsondb_ut_err_{ts}"));
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
fn open_missing_db_fails_and_create_twice_fails() {
    let tmp = tmp_root();
    std::env::set_var("PATH_GENAPTITUDE_DOMAIN", &tmp);

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = find_repo_root(&manifest);
    let cfg = JsonDbConfig::from_env(&repo_root).expect("cfg");

    let (space, db) = ("un2", "_system");

    // open sur DB inexistante → Err
    assert!(open_db(&cfg, space, db).is_err());

    // create puis create à nouveau → Err
    create_db(&cfg, space, db).expect("create");
    assert!(create_db(&cfg, space, db).is_err());

    // cleanup
    drop_db(&cfg, space, db, DropMode::Hard).ok();
    let _ = fs::remove_dir_all(tmp);
}
