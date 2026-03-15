use crate::utils::prelude::*;

use super::intent_classifier::EngineeringIntent;
// ✅ IMPORT DU MODULE TOOLS FACTORISÉ
use super::tools::{
    extract_json_from_llm, load_session, query_knowledge_graph, save_artifact, save_session,
};
use super::{Agent, AgentContext, AgentResult};

// Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct DataAgent;

impl DataAgent {
    pub fn new() -> Self {
        Self {}
    }

    /// Détermine quel agent doit être notifié de la création de cette donnée
    fn determine_target_agent(&self, name: &str) -> (&'static str, String) {
        let lower = name.to_lowercase();

        if lower.contains("client") || lower.contains("business") || lower.contains("metier") {
            ("business_analyst", format!("J'ai défini la donnée métier '{}'. Peux-tu vérifier si elle est utilisée dans les processus OA ?", name))
        } else if lower.contains("signal")
            || lower.contains("voltage")
            || lower.contains("hardware")
        {
            ("hardware_architect", format!("J'ai défini le signal physique '{}'. Merci de vérifier la compatibilité avec les interfaces matérielles.", name))
        } else if lower.contains("config")
            || lower.contains("param")
            || lower.contains("settings")
            || lower.contains("log")
        // ✅ AJOUT : Les logs sont du domaine software
        {
            ("software_engineer", format!("J'ai défini la structure logicielle '{}'. Peux-tu l'implémenter dans le code ?", name))
        } else {
            ("system_architect", format!("J'ai défini la donnée système '{}'. Elle est disponible pour les échanges fonctionnels.", name))
        }
    }

    async fn call_llm(
        &self,
        ctx: &AgentContext,
        sys: &str,
        user: &str,
        doc_type: &str,
        original_name: &str,
    ) -> RaiseResult<JsonValue> {
        let response = ctx.llm.ask(LlmBackend::LocalLlama, sys, user).await?;

        let clean = extract_json_from_llm(&response);
        let mut doc: JsonValue = json::deserialize_from_str(&clean).unwrap_or(json_value!({}));

        // --- BLINDAGE TOTAL ---
        doc["name"] = json_value!(original_name);
        doc["id"] = json_value!(UniqueId::new_v4().to_string());
        doc["layer"] = json_value!("DATA");
        doc["type"] = json_value!(doc_type);
        doc["createdAt"] = json_value!(UtcClock::now().to_rfc3339());

        if doc_type == "Class" && doc.get("attributes").is_none() {
            doc["attributes"] = json_value!([]);
        }
        Ok(doc)
    }

    async fn enrich_class(
        &self,
        ctx: &AgentContext,
        name: &str,
        history_context: &str,
    ) -> RaiseResult<JsonValue> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Attributs potentiels :\n");
            for e in entities {
                nlp_hint.push_str(&format!("- {}\n", e.text));
            }
        }
        let sys = "Tu es Data Architect. JSON Strict.";
        let user = format!(
            "=== HISTORIQUE ===\n{}\n\n=== TÂCHE ===\nNom: {}\n{}\nJSON: {{ \"name\": \"{}\", \"attributes\": [] }}",
            history_context, name, nlp_hint, name
        );
        self.call_llm(ctx, sys, &user, "Class", name).await
    }
}

#[async_interface]
impl Agent for DataAgent {
    fn id(&self) -> &'static str {
        "data_architect"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>> {
        // 1. CHARGEMENT SESSION
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "DATA" => {
                session.add_message(
                    "user",
                    &format!("Create Data Element: {} ({})", name, element_type),
                );

                // --- 🎯 ÉTAPE : VÉRIFICATION SÉMANTIQUE ---
                let slug = name.to_lowercase().replace(" ", "_");

                // ✅ CORRECTIF : On utilise un format d'URN compatible ref:<collection>:<id>
                let element_type_lower = element_type.to_lowercase();
                let collection_name = match element_type_lower.as_str() {
                    "class" | "classe" | "classes" => "classes",
                    "component" | "composant" | "components" => "components",
                    other => other,
                };

                // On retire le préfixe "data:" qui perturbait la résolution par collection
                let reference = format!("ref:{}:_id:{}", collection_name, slug);

                if let Ok(existing) = query_knowledge_graph(ctx, &reference, false).await {
                    // ✅ CORRECTIF COMPILATION : On vérifie les variantes spécifiquement
                    let is_real_data = match &existing {
                        JsonValue::Object(o) => !o.is_empty(),
                        JsonValue::Array(a) => !a.is_empty(),
                        JsonValue::String(s) => !s.is_empty(),
                        JsonValue::Null => false,
                        _ => true, // Booléens ou Nombres sont considérés comme des données présentes
                    };

                    if is_real_data {
                        let msg = format!(
                            "La donnée **{}** est déjà définie dans le modèle (Reference: `{}`). Création ignorée.",
                            name,
                            reference
                        );
                        session.add_message("assistant", &msg);
                        save_session(ctx, &session).await?;
                        return Ok(Some(AgentResult::text(msg)));
                    }
                }

                // Calcul Historique pour l'enrichissement LLM
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let (doc, collection) = match element_type.to_lowercase().as_str() {
                    "class" | "classe" => {
                        (self.enrich_class(ctx, name, &history_str).await?, "classes")
                    }
                    _ => (self.enrich_class(ctx, name, &history_str).await?, "classes"),
                };

                let artifact = save_artifact(ctx, "data", collection, &doc).await?;

                // 2. ROUTAGE DYNAMIQUE
                let (target_agent, msg_content) = self.determine_target_agent(name);

                let acl_msg =
                    AclMessage::new(Performative::Inform, self.id(), target_agent, &msg_content);

                let msg = format!(
                    "Donnée **{}** définie. Notification envoyée à l'agent **{}**.",
                    name, target_agent
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: Some(acl_msg),
                }))
            }
            _ => Ok(None),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION SÉMANTIQUE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::llm::client::LlmClient;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[test]
    fn test_data_agent_id() {
        assert_eq!(DataAgent::new().id(), "data_architect");
    }

    #[test]
    fn test_data_routing_logic() {
        let agent = DataAgent::new();

        let (t1, _) = agent.determine_target_agent("Business_Invoice");
        assert_eq!(t1, "business_analyst");

        let (t2, _) = agent.determine_target_agent("Voltage_Sensor_Reading");
        assert_eq!(t2, "hardware_architect");

        // ✅ TEST CORRIGÉ : System_Log_Level doit maintenant router vers software_engineer
        let (t3, _) = agent.determine_target_agent("System_Log_Level");
        assert_eq!(t3, "software_engineer");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_data_duplicate_prevention_integration() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // ✅ SEED : On injecte bien dans "classes" pour correspondre à la résolution sémantique
        let existing_data = json_value!({
            "_id": "user_profile",
            "name": "User Profile",
            "type": "Class",
            "layer": "DATA"
        });

        manager
            .create_collection(
                "classes",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document("classes", existing_data)
            .await
            .unwrap();

        inject_mock_component(
            &manager,
            "llm",
            json_value!({
                "rust_tokenizer_file": "tokenizer.json",
                "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf"
            }),
        )
        .await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let ctx = AgentContext::new(
            "test_user",
            "sess_data_01",
            sandbox.db.clone(),
            llm,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        let agent = DataAgent::new();

        let intent = EngineeringIntent::CreateElement {
            layer: "DATA".to_string(),
            element_type: "Class".to_string(),
            name: "User Profile".to_string(),
        };

        let result = agent.process(&ctx, &intent).await.unwrap();

        if let Some(res) = result {
            // ✅ VÉRIFICATION : L'agent doit avoir trouvé le doublon
            assert!(
                res.message.contains("déjà définie"),
                "L'agent n'a pas détecté le doublon. Message reçu: {}",
                res.message
            );
            assert!(res.artifacts.is_empty());
        } else {
            panic!("L'agent aurait dû renvoyer un résultat.");
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_data_session_persistence() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(
            &manager,
            "llm",
            json_value!({
                "rust_tokenizer_file": "tokenizer.json",
                "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf"
            }),
        )
        .await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let ctx = AgentContext::new(
            "test_user",
            "sess_persist_01",
            sandbox.db.clone(),
            llm,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        let mut session = super::load_session(&ctx).await.unwrap();
        session.add_message("user", "Ping");
        super::save_session(&ctx, &session).await.unwrap();

        let session_reload = super::load_session(&ctx).await.unwrap();
        assert_eq!(session_reload.messages.len(), 1);
        assert_eq!(session_reload.messages[0].content, "Ping");
    }
}
