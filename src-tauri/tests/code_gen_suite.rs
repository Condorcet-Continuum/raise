// src-tauri/tests/code_gen_suite.rs

// Environnement commun
#[path = "code_gen_suite/mod.rs"]
mod common;

// Tests de génération pure (Symbolique)
#[path = "code_gen_suite/rust_tests.rs"]
mod rust_tests;

// Tests d'intégration avec l'Agent (Neuronal)
#[path = "code_gen_suite/agent_tests.rs"]
mod agent_tests;
