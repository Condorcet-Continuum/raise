use crate::model_engine::types::ProjectModel;
use anyhow::Result;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct CapellaWriter;

impl CapellaWriter {
    /// Sauvegarde le modèle au format JSON (RAISE native format)
    /// Nous n'écrivons pas en .capella (XMI) pour l'instant car c'est trop risqué sans EMF.
    pub fn save_as_json(model: &ProjectModel, path: &Path) -> Result<()> {
        let json_data = serde_json::to_string_pretty(model)?;
        let mut file = File::create(path)?;
        file.write_all(json_data.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_json() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("model.json");

        let model = ProjectModel::default();
        // Le test passe si aucune erreur n'est levée
        CapellaWriter::save_as_json(&model, &file_path).expect("Save failed");

        assert!(file_path.exists());
    }
}
