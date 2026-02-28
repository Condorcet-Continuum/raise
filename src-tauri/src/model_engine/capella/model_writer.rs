use crate::model_engine::types::ProjectModel;
use crate::utils::{data, io, prelude::*};

pub struct CapellaWriter;

impl CapellaWriter {
    /// Sauvegarde le modèle au format JSON (RAISE native format) de manière asynchrone et atomique.
    pub async fn save_as_json(model: &ProjectModel, path: &Path) -> RaiseResult<()> {
        // 1. Sérialisation JSON (déjà asynchrone/RaiseResult via nos utils)
        let json_data = data::stringify_pretty(model)?;

        // 2. Écriture atomique (via utils::io::write_atomic)
        io::write_atomic(path, json_data.as_bytes()).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::tempdir;

    #[tokio::test] // On passe le test en tokio::test pour l'async
    async fn test_save_json() {
        let dir = tempdir().unwrap(); // tempdir() est aussi async
        let file_path = dir.path().join("model.json");

        let model = ProjectModel::default();

        // Appel async avec .await
        CapellaWriter::save_as_json(&model, &file_path)
            .await
            .expect("Save failed");

        assert!(file_path.exists());
    }
}
