use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use surrealdb::engine::local::{Db, SurrealKv};
use surrealdb::Surreal;

#[derive(Clone)]
pub struct SurrealClient {
    db: Surreal<Db>,
}

impl SurrealClient {
    pub async fn init(data_dir: PathBuf) -> Result<Self> {
        let db_path = data_dir.join("raise_graph.db");
        let db = Surreal::new::<SurrealKv>(db_path).await?;
        db.use_ns("raise").use_db("graph").await?;
        Ok(Self { db })
    }

    /// Insert ou Met à jour un nœud
    pub async fn upsert_node(
        &self,
        table: &str,
        id: &str,
        content: Value,
    ) -> Result<Option<Value>> {
        let json_content = serde_json::to_string(&content)?;

        // ASTUCE CLEF : On utilise "RETURN *, <string>id as id"
        // Cela convertit le 'Thing' (objet ID binaire) en String simple "table:id"
        // Le désérialiseur JSON standard peut alors le lire sans erreur.
        let create_sql = format!(
            "CREATE type::thing('{}', '{}') CONTENT {} RETURN *, <string>id as id;",
            table, id, json_content
        );

        let mut res = self.db.query(&create_sql).await?;

        // On récupère directement en serde_json::Value (maintenant que l'ID est une string)
        if let Ok(Some(val)) = res.take::<Option<Value>>(0) {
            return Ok(Some(val));
        }

        // Fallback: UPDATE
        let update_sql = format!(
            "UPDATE type::thing('{}', '{}') CONTENT {} RETURN *, <string>id as id;",
            table, id, json_content
        );

        let mut res_update = self.db.query(&update_sql).await?;
        let updated: Option<Value> = res_update.take(0)?;
        Ok(updated)
    }

    pub async fn select(&self, table: &str, id: &str) -> Result<Option<Value>> {
        // Idem : On cast l'ID en string dès la requête
        let sql = format!(
            "SELECT *, <string>id as id FROM type::thing('{}', '{}');",
            table, id
        );
        let mut res = self.db.query(&sql).await?;
        let record: Option<Value> = res.take(0)?;
        Ok(record)
    }

    pub async fn delete_node(&self, table: &str, id: &str) -> Result<()> {
        let sql = format!("DELETE type::thing('{}', '{}');", table, id);
        // CORRECTION : On ignore totalement le résultat (qui est un booléen 'true')
        // pour ne pas crasher le désérialiseur qui attendrait un objet/null.
        let _ = self.db.query(&sql).await?;
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

        let _ = self.db.query(sql).await?;
        Ok(())
    }

    pub async fn search_similar(
        &self,
        table: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<Value>> {
        let vector_json = serde_json::to_string(&vector)?;

        // Ici aussi, on cast l'ID en string pour sécuriser la sortie
        let sql = format!(
            "SELECT *, <string>id as id, vector::similarity::cosine(embedding, {}) AS score 
             FROM type::table('{}') ORDER BY score DESC LIMIT {};",
            vector_json, table, limit
        );

        let mut response = self.db.query(sql).await?;
        let results: Vec<Value> = response.take(0)?;
        Ok(results)
    }

    pub async fn raw_query(&self, query: &str) -> Result<Vec<Value>> {
        // Pour raw_query, on ne peut pas injecter le cast "<string>id" automatiquement.
        // Si ça plante ici, l'utilisateur devra ajouter le cast dans son SQL.
        let mut res = self.db.query(query).await?;

        // On essaie de récupérer en JSON standard.
        // Si ça échoue à cause d'un ID binaire, c'est une limitation connue en Embedded sans cast.
        // Mais pour les tests "SELECT ->working_on...", les résultats sont souvent des projections
        // sans l'ID brut racine, donc ça peut passer.
        let results: Vec<Value> = res.take(0)?;
        Ok(results)
    }
}

// =========================================================================
// TESTS
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use std::fs;

    async fn setup_test_db(test_name: &str) -> (SurrealClient, PathBuf) {
        let mut temp_path = env::temp_dir();
        temp_path.push("raise_tests");
        temp_path.push(test_name);

        if temp_path.exists() {
            let _ = fs::remove_dir_all(&temp_path);
        }
        fs::create_dir_all(&temp_path).expect("Impossible de créer le dossier temp");

        let client = SurrealClient::init(temp_path.clone())
            .await
            .expect("Echec init DB");
        (client, temp_path)
    }

    fn teardown_test_db(path: PathBuf) {
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn test_upsert_and_read_node() {
        let (client, path) = setup_test_db("test_upsert").await;
        let table = "test_func";
        let id = "f1";

        let data = json!({ "name": "My Function", "complexity": 5 });

        let res = client.upsert_node(table, id, data.clone()).await;
        assert!(
            res.is_ok(),
            "Premier upsert (create) a échoué : {:?}",
            res.err()
        );
        assert!(res.as_ref().unwrap().is_some());

        let data2 = json!({ "name": "My Function Updated", "complexity": 10 });
        let res2 = client.upsert_node(table, id, data2).await;
        assert!(res2.is_ok(), "Second upsert (update) a échoué");

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

        client
            .upsert_node(t_person, "alice", json!({"role": "admin"}))
            .await
            .unwrap();
        client
            .upsert_node(t_project, "raise", json!({"status": "dev"}))
            .await
            .unwrap();

        let res = client
            .create_edge((t_person, "alice"), "working_on", (t_project, "raise"))
            .await;
        res.expect("Échec critique lors de la création de la relation");

        let raw_query = format!(
            "SELECT ->working_on->{}.{{ status }} as projects FROM {}:alice",
            t_project, t_person
        );

        let rows = client.raw_query(&raw_query).await.unwrap();

        assert!(!rows.is_empty());
        let projects = rows[0]["projects"].as_array();
        assert!(projects.is_some(), "Projects field missing");
        let projects_arr = projects.unwrap();
        assert!(!projects_arr.is_empty());

        // Le test vérifie "status", donc tout ira bien
        assert_eq!(projects_arr[0]["status"], "dev");

        teardown_test_db(path);
    }

    #[tokio::test]
    async fn test_vector_search_mock() {
        let (client, path) = setup_test_db("test_vector").await;
        let table = "test_mem";

        let doc1 = json!({ "content": "Hello", "embedding": [0.1, 0.2, 0.3] });
        client.upsert_node(table, "m1", doc1).await.unwrap();

        let res = client.search_similar(table, vec![0.1, 0.2, 0.3], 1).await;
        assert!(res.is_ok());
        assert!(!res.unwrap().is_empty());

        teardown_test_db(path);
    }
}
