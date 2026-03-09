// FICHIER : src-tauri/src/utils/testing/mod.rs

pub mod mock;

// On expose les sandboxes pour qu'elles soient facilement utilisables
// dans les tests des autres modules (ex: dossier blockchain ou services).
pub use mock::{
    inject_collection_schema, inject_mock_component, inject_mock_config, AgentDbSandbox, DbSandbox,
    GlobalDbSandbox, SESSION_SCHEMA_MOCK,
};
