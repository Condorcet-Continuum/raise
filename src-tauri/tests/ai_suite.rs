// src-tauri/tests/ai_suite.rs

// Module commun (Setup, Helpers)
#[path = "ai_suite/mod.rs"]
mod common;

// Tests de connectivité LLM (Ping, Dual Mode)
#[path = "ai_suite/llm_tests.rs"]
mod llm_tests;

// --- TESTS DES AGENTS (CYCLE EN V ARCADIA) ---

// OA : Business Agent (Operational Analysis)
#[path = "ai_suite/business_agent_tests.rs"]
mod business_agent_tests;

// SA : System Agent (System Analysis)
#[path = "ai_suite/system_agent_tests.rs"]
mod system_agent_tests;

// LA : Software Agent (Logical Architecture)
#[path = "ai_suite/software_agent_tests.rs"]
mod software_agent_tests;

// PA : Hardware Agent (Physical Architecture)
#[path = "ai_suite/hardware_agent_tests.rs"]
mod hardware_agent_tests;

// EPBS : Configuration Items
#[path = "ai_suite/epbs_agent_tests.rs"]
mod epbs_agent_tests;

// DATA : Master Data Management
#[path = "ai_suite/data_agent_tests.rs"]
mod data_agent_tests;

// TRANSVERSE : Exigences, Tests & IVVQ
#[path = "ai_suite/transverse_agent_tests.rs"]
mod transverse_agent_tests;

// TRANSVERSE : Exigences, Tests & IVVQ
#[path = "ai_suite/deep_learning_tests.rs"]
mod deep_learning_tests;
