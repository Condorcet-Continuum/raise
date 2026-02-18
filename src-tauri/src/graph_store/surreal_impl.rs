use crate::utils::{data, io::PathBuf, prelude::*};

use surrealdb::engine::local::{Db, SurrealKv};
use surrealdb::Surreal;

#[derive(Clone)]
pub struct SurrealClient {
    db: Surreal<Db>,
}

impl SurrealClient {
    pub async fn init(data_dir: PathBuf) -> Result<Self> {
        let db_path = data_dir.join("raise_graph.db");
        let db = Surreal::new::<SurrealKv>(db_path)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;

        db.use_ns("raise")
            .use_db("graph")
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(Self { db })
    }

    /// Insert ou Met à jour un nœud (Idempotent)
    pub async fn upsert_node(
        &self,
        table: &str,
        id: &str,
        content: Value,
    ) -> Result<Option<Value>> {
        let json_content = data::stringify(&content)?;

        // 1. TENTATIVE DE CRÉATION
        // "RETURN *, <string>id as id" force l'ID en format "table:id" (String) pour la compatibilité JSON
        let create_sql = format!(
            "CREATE type::thing('{}', '{}') CONTENT {} RETURN *, <string>id as id;",
            table, id, json_content
        );

        // On capture le résultat global (Result<Response>)
        let res = self.db.query(&create_sql).await;

        match res {
            Ok(mut response) => {
                // take(0) renvoie un Result. Si Ok, c'est que le CREATE a marché.
                // Si le record existe déjà, SurrealDB renvoie une erreur ici ou dans la query.
                if let Ok(Some(val)) = response.take::<Option<Value>>(0) {
                    return Ok(Some(val));
                }
            }
            Err(_) => {
                // Erreur probable : L'enregistrement existe déjà (Conflit ID).
                // On passe au fallback (UPDATE).
            }
        }

        // 2. FALLBACK : UPDATE (Si l'ID existe déjà)
        let update_sql = format!(
            "UPDATE type::thing('{}', '{}') CONTENT {} RETURN *, <string>id as id;",
            table, id, json_content
        );

        let mut res_update = self
            .db
            .query(&update_sql)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        let updated: Option<Value> = res_update
            .take(0)
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(updated)
    }

    pub async fn select(&self, table: &str, id: &str) -> Result<Option<Value>> {
        let sql = format!(
            "SELECT *, <string>id as id FROM type::thing('{}', '{}');",
            table, id
        );
        let mut res = self
            .db
            .query(&sql)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        let record: Option<Value> = res.take(0).map_err(|e| AppError::from(e.to_string()))?;
        Ok(record)
    }

    pub async fn delete_node(&self, table: &str, id: &str) -> Result<()> {
        let sql = format!("DELETE type::thing('{}', '{}');", table, id);
        let _ = self
            .db
            .query(&sql)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(())
    }

    pub async fn create_edge(
        &self,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> Result<()> {
        let from_record = format!("{}:{}", from.0, from.1);
        let to_record = format!("{}:{}", to.0, to.1);

        let sql = format!(
            "RELATE {} -> {} -> {} RETURN NONE;",
            from_record, relation, to_record
        );

        let _ = self
            .db
            .query(sql)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(())
    }

    pub async fn search_similar(
        &self,
        table: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let vector_json = data::stringify(&vector)?;

        let sql = format!(
            "SELECT *, <string>id as id, vector::similarity::cosine(embedding, {}) AS score 
             FROM type::table('{}') 
             WHERE embedding != NONE 
             ORDER BY score DESC LIMIT {};",
            vector_json, table, limit
        );

        let mut response = self
            .db
            .query(sql)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        let results: Vec<Value> = response
            .take(0)
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(results)
    }

    #[allow(dead_code)]
    pub async fn raw_query(&self, query: &str) -> Result<Vec<Value>> {
        let mut res = self
            .db
            .query(query)
            .await
            .map_err(|e| AppError::from(e.to_string()))?;
        let results: Vec<Value> = res.take(0).map_err(|e| AppError::from(e.to_string()))?;
        Ok(results)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::{self, PathBuf};
    use std::env;

    // Helper pour créer une DB isolée pour chaque test
    async fn setup_test_db(test_name: &str) -> (SurrealClient, PathBuf) {
        let mut temp_path = env::temp_dir();
        temp_path.push("raise_tests");
        temp_path.push(test_name);

        // Nettoyage préalable
        if temp_path.exists() {
            let _ = io::remove_dir_all(&temp_path).await;
        }
        io::create_dir_all(&temp_path)
            .await
            .expect("Impossible de créer le dossier temp");

        let client = SurrealClient::init(temp_path.clone())
            .await
            .expect("Echec init DB");
        (client, temp_path)
    }

    // Helper pour nettoyer après le test
    fn teardown_test_db(path: PathBuf) {
        let _ = io::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn test_upsert_and_read_node() {
        let (client, path) = setup_test_db("test_upsert").await;
        let table = "test_func";
        let id = "f1";

        // 1. Premier Upsert (Create)
        let data = json!({ "name": "My Function", "complexity": 5 });
        let res = client.upsert_node(table, id, data.clone()).await;

        assert!(res.is_ok(), "Premier upsert échoué: {:?}", res.err());
        assert!(res.as_ref().unwrap().is_some());

        // 2. Second Upsert (Update)
        let data2 = json!({ "name": "My Function Updated", "complexity": 10 });
        let res2 = client.upsert_node(table, id, data2).await;
        assert!(res2.is_ok(), "Second upsert échoué");

        // 3. Vérification lecture
        let node = client.select(table, id).await.unwrap();
        assert_eq!(node.unwrap()["name"], "My Function Updated");

        teardown_test_db(path);
    }

    #[tokio::test]
    async fn test_delete_node() {
        let (client, path) = setup_test_db("test_delete").await;
        let table = "test_comp";

        client
            .upsert_node(table, "c1", json!({"active": true}))
            .await
            .unwrap();
        assert!(client.select(table, "c1").await.unwrap().is_some());

        let res = client.delete_node(table, "c1").await;
        assert!(res.is_ok());
        assert!(client.select(table, "c1").await.unwrap().is_none());

        teardown_test_db(path);
    }

    #[tokio::test]
    async fn test_graph_relations() {
        let (client, path) = setup_test_db("test_relations").await;
        let t_person = "test_person";
        let t_project = "test_proj";

        // Création des nœuds
        client
            .upsert_node(t_person, "alice", json!({"role": "admin"}))
            .await
            .unwrap();
        client
            .upsert_node(t_project, "raise", json!({"status": "dev"}))
            .await
            .unwrap();

        // Création relation : Alice -> working_on -> Raise
        let res = client
            .create_edge((t_person, "alice"), "working_on", (t_project, "raise"))
            .await;
        res.expect("Échec création relation");

        // Requête Graphique : "Quels projets pour Alice ?"
        let raw_query = format!(
            "SELECT ->working_on->{}.{{ status }} as projects FROM {}:alice",
            t_project, t_person
        );

        let rows = client.raw_query(&raw_query).await.unwrap();

        assert!(!rows.is_empty());
        let projects = rows[0]["projects"].as_array().expect("Projects missing");
        assert!(!projects.is_empty());
        assert_eq!(projects[0]["status"], "dev");

        teardown_test_db(path);
    }

    #[tokio::test]
    async fn test_vector_search_mock() {
        let (client, path) = setup_test_db("test_vector").await;
        let table = "test_mem";

        // Insertion d'un vecteur mocké
        let doc1 = json!({ "content": "Hello", "embedding": [0.1, 0.2, 0.3] });
        client.upsert_node(table, "m1", doc1).await.unwrap();

        // Recherche
        let res = client.search_similar(table, vec![0.1, 0.2, 0.3], 1).await;
        assert!(res.is_ok());
        assert!(!res.unwrap().is_empty());

        teardown_test_db(path);
    }
}
