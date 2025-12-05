// src-tauri/tests/ai_suite.rs

// Module commun (Setup, Helpers)
#[path = "ai_suite/mod.rs"]
mod common;

// Tests de connectivité LLM (Ping, Dual Mode)
#[path = "ai_suite/llm_tests.rs"]
mod llm_tests;

// Tests des Agents (Scénarios complets : Instruction -> DB)
#[path = "ai_suite/agent_tests.rs"]
mod agent_tests;
