// FICHIER : src-tauri/tests/genetics_suite.rs

// ✅ On rend le module 'common' accessible à cette suite de tests
#[path = "common/mod.rs"]
mod common;

// Déclaration des sous-modules existants
// Note : Le dossier physique semble être "tests/genetics_suite/"
// donc ces modules correspondent aux fichiers .rs dans ce dossier.
#[path = "genetics_suite/full_flow_test.rs"]
pub mod full_flow_test;

#[path = "genetics_suite/integration_test.rs"]
pub mod integration_test;
