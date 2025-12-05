use crate::ai::agents::{Agent, EngineeringIntent};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::json;

pub struct SystemAgent {
    llm: LlmClient,
    storage: StorageEngine,
    space: String,
    db: String,
}

impl SystemAgent {
    pub fn new(llm: LlmClient, storage: StorageEngine) -> Self {
        Self {
            llm,
            storage,
            space: "un2".to_string(),
            db: "_system".to_string(),
        }
    }

    /// Recherche intelligente : gère la casse et les articles (Le, La...)
    fn find_id_by_name(&self, collection: &str, name: &str) -> Result<Option<String>> {
        let mgr = CollectionsManager::new(&self.storage, &self.space, &self.db);

        // Si la collection n'existe pas encore, on ne peut rien trouver
        if !self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .exists()
        {
            return Ok(None);
        }

        let docs = mgr.list_all(collection)?;

        // Normalisation de la requête : "Le Superviseur" -> "superviseur"
        let clean_search = name
            .to_lowercase()
            .replace("le ", "")
            .replace("la ", "")
            .replace("l'", "")
            .trim()
            .to_string();

        for doc in docs {
            if let Some(doc_name) = doc.get("name").and_then(|v| v.as_str()) {
                let clean_doc_name = doc_name.to_lowercase();

                // Match exact ou match "nettoyé"
                if clean_doc_name == name.to_lowercase() || clean_doc_name == clean_search {
                    let id = doc.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    return Ok(Some(id.to_string()));
                }
            }
        }
        Ok(None)
    }

    async fn generate_description(&self, element_type: &str, name: &str) -> String {
        let prompt = format!(
            "Génère une description technique courte (1 phrase) pour un '{}' nommé '{}' dans un contexte d'ingénierie système. \
            Réponds impérativement en Français. Sois factuel.",
            element_type, name
        );

        self.llm
            .ask(
                LlmBackend::LocalLlama,
                "Tu es un assistant ingénieur.",
                &prompt,
            )
            .await
            .unwrap_or_else(|_| "Description générée par IA.".to_string())
    }
}

#[async_trait]
impl Agent for SystemAgent {
    async fn process(&self, intent: &EngineeringIntent) -> Result<Option<String>> {
        match intent {
            // --- CAS 1 : CRÉATION D'ÉLÉMENT ---
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } => {
                if layer != "OA" && layer != "SA" {
                    return Ok(None);
                }

                println!("🤖 SystemAgent: Création de {} {}...", layer, name);

                // Mapping des schémas (Corrects)
                let (collection, type_uri, schema_rel) =
                    match (layer.as_str(), element_type.as_str()) {
                        ("OA", "Actor") => (
                            "actors",
                            "oa:OperationalActor",
                            "arcadia/oa/actor.schema.json",
                        ),
                        ("OA", "Activity") => (
                            "activities",
                            "oa:OperationalActivity",
                            "arcadia/oa/activity.schema.json",
                        ),
                        ("SA", "Function") => (
                            "functions",
                            "sa:SystemFunction",
                            "arcadia/sa/system-function.schema.json",
                        ),
                        ("SA", "Component") => (
                            "components",
                            "sa:SystemComponent",
                            "arcadia/sa/system-component.schema.json",
                        ),
                        _ => return Err(anyhow!("Type non supporté: {}/{}", layer, element_type)),
                    };

                // Check doublon
                if let Ok(Some(existing_id)) = self.find_id_by_name(collection, name) {
                    return Ok(Some(format!(
                        "⚠️ L'élément **{}** existe déjà (ID: `{}`).",
                        name, existing_id
                    )));
                }

                let schema_uri =
                    format!("db://{}/{}/schemas/v1/{}", self.space, self.db, schema_rel);
                let document_schema_ref = format!("../../../schemas/v1/{}", schema_rel);
                let description = self.generate_description(element_type, name).await;

                let doc = json!({
                    "$schema": document_schema_ref,
                    "@context": {
                        "oa": "https://genaptitude.io/ontology/arcadia/oa#",
                        "sa": "https://genaptitude.io/ontology/arcadia/sa#",
                        "name": "http://www.w3.org/2004/02/skos/core#prefLabel"
                    },
                    "@type": type_uri,
                    "name": name,
                    "description": description,
                    "status": "draft_ai"
                });

                let mgr = CollectionsManager::new(&self.storage, &self.space, &self.db);
                mgr.create_collection(collection, Some(schema_uri.clone()))?;
                let saved = mgr.insert_with_schema(collection, doc)?;
                let id = saved.get("id").and_then(|v| v.as_str()).unwrap_or("?");

                Ok(Some(format!(
                    "✅ Créé : **{}** (Type: {}).\nID: `{}`",
                    name, element_type, id
                )))
            }

            // --- CAS 2 : CRÉATION DE RELATION (Implémenté !) ---
            EngineeringIntent::CreateRelationship {
                source_name,
                target_name,
                relation_type,
            } => {
                println!(
                    "🤖 SystemAgent: Link '{}' -> '{}' ({})",
                    source_name, target_name, relation_type
                );

                let mgr = CollectionsManager::new(&self.storage, &self.space, &self.db);

                // 1. Trouver la Source (On cherche large : Acteurs ou Composants)
                let (source_id, source_coll) =
                    if let Some(id) = self.find_id_by_name("actors", source_name)? {
                        (id, "actors")
                    } else if let Some(id) = self.find_id_by_name("components", source_name)? {
                        (id, "components")
                    } else {
                        return Ok(Some(format!(
                            "❌ Impossible de trouver l'élément source **{}**.",
                            source_name
                        )));
                    };

                // 2. Trouver la Cible (Activités ou Fonctions)
                let target_id = if let Some(id) = self.find_id_by_name("activities", target_name)? {
                    id
                } else if let Some(id) = self.find_id_by_name("functions", target_name)? {
                    id
                } else {
                    return Ok(Some(format!(
                        "❌ Impossible de trouver l'élément cible **{}**.",
                        target_name
                    )));
                };

                // 3. Charger et Modifier
                let mut source_doc = mgr
                    .get(source_coll, &source_id)?
                    .context("Doc source perdu")?;

                // Choix du champ selon le type (OA vs SA)
                // OA: Actor -> allocatedActivities
                // SA: Component -> allocatedFunctions
                let field_name = if source_coll == "actors" {
                    "allocatedActivities"
                } else {
                    "allocatedFunctions"
                };

                if let Some(obj) = source_doc.as_object_mut() {
                    if !obj.contains_key(field_name) {
                        obj.insert(field_name.to_string(), json!([]));
                    }

                    if let Some(list) = obj.get_mut(field_name).and_then(|v| v.as_array_mut()) {
                        let link_obj = json!({ "id": target_id });
                        // Anti-doublon
                        if !list
                            .iter()
                            .any(|x| x.get("id").and_then(|v| v.as_str()) == Some(&target_id))
                        {
                            list.push(link_obj);
                        } else {
                            return Ok(Some(format!("ℹ️ Le lien existe déjà.")));
                        }
                    }
                }

                // 4. Sauvegarder
                mgr.update_document(source_coll, &source_id, source_doc)?;

                Ok(Some(format!(
                    "🔗 Relation établie : **{}** -> **{}**.",
                    source_name, target_name
                )))
            }

            _ => Ok(None),
        }
    }
}
