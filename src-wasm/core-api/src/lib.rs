use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================
// 1. INTERFACE SYSTÈME (Mise à jour pour la lecture DB)
// =============================================================

extern "C" {
    fn host_log(ptr: *const u8, len: usize);
    // 👇 NOUVEAU : La fonction pour lire la DB
    fn host_db_read(ptr: *const u8, len: usize) -> i32;
}

pub fn log(message: &str) {
    unsafe {
        host_log(message.as_ptr(), message.len());
    }
}

/// Tente de lire un document dans la base de données de l'hôte.
/// Pour l'instant, cela déclenche l'affichage de la donnée côté Host.
pub fn db_read(collection: &str, id: &str) -> bool {
    // On prépare la requête au format attendu par cognitive.rs
    let request = serde_json::json!({
        "collection": collection,
        "id": id
    })
    .to_string();

    unsafe {
        // On envoie la requête. Si host_db_read renvoie 1, c'est un succès.
        host_db_read(request.as_ptr(), request.len()) == 1
    }
}

// =============================================================
// 2. STRUCTURES DE DONNÉES (INCHANGÉES)
// =============================================================

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CognitiveModel {
    pub id: String,
    pub elements: HashMap<String, ModelElement>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelElement {
    pub name: String,
    pub kind: String,
    pub properties: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AnalysisReport {
    pub block_id: String,
    pub status: AnalysisStatus,
    pub messages: Vec<String>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AnalysisStatus {
    Success,
    Warning,
    Failure,
}

pub trait CognitiveBlock {
    fn id(&self) -> &str;
    fn execute(&self, model: &CognitiveModel) -> Result<AnalysisReport, String>;
}
