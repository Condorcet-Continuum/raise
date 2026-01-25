// src-tauri/src/ai/protocols/mod.rs

pub mod acl;
pub mod mcp;

// On réexporte les types principaux pour faciliter l'usage dans le reste de l'app
pub use acl::{AclMessage, Performative};
pub use mcp::{McpToolCall, McpToolResult};
