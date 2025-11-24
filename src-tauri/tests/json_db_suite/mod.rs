// src-tauri/tests/json_db_suite/mod.rs

// On ré-exporte simplement ce qui vient de la lib principale.
// "genaptitude" est le nom déclaré dans [package] name = "genaptitude" de Cargo.toml

pub use genaptitude::json_db::test_utils::{
    ensure_db_exists, get_dataset_root, init_test_env, TEST_DB, TEST_SPACE,
};
