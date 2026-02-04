// FICHIER : src-tauri/src/ai/tools/mod.rs

pub mod codegen_tool;
pub mod file_system;
// Export pour faciliter l'usage
pub use codegen_tool::CodeGenTool;
pub use file_system::FileWriteTool;
