use crate::utils::{json, AppError, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::fs;

use tracing::{debug, error, instrument};

// --- RE-EXPORTS (Pour éviter les imports std::path ailleurs) ---
pub use include_dir::{include_dir, Dir};
pub use std::path::{Component, Path, PathBuf};
pub use walkdir::WalkDir;

// --- LECTURE ---
pub use tokio::fs::{DirEntry, File, ReadDir}; // Permet .write_all() et .flush()
pub use tokio::io::AsyncWriteExt;

pub use tempfile::{tempdir, TempDir};

pub async fn create_dir_all(path: impl AsRef<std::path::Path>) -> crate::utils::Result<()> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| crate::utils::AppError::System(anyhow::anyhow!(e)))
}

pub async fn copy_dir_all(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
) -> crate::utils::Result<()> {
    let src = src.as_ref().to_path_buf();
    let dst = dst.as_ref().to_path_buf();

    // On utilise tokio::fs::try_exists ou notre propre fonction exists()
    if !tokio::fs::try_exists(&src).await.unwrap_or(false) {
        // CORRECTION 1 : AppError::Io (et non IO)
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Source directory not found: {:?}", src),
        )));
    }

    if !tokio::fs::try_exists(&dst).await.unwrap_or(false) {
        tokio::fs::create_dir_all(&dst)
            .await
            .map_err(AppError::Io)?;
    }

    let mut stack = vec![(src, dst)];

    while let Some((current_src, current_dst)) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current_src)
            .await
            .map_err(AppError::Io)?;

        while let Some(entry) = entries.next_entry().await.map_err(AppError::Io)? {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let dest_path = current_dst.join(entry_name);
            let file_type = entry.file_type().await.map_err(AppError::Io)?;

            if file_type.is_dir() {
                if !tokio::fs::try_exists(&dest_path).await.unwrap_or(false) {
                    tokio::fs::create_dir_all(&dest_path)
                        .await
                        .map_err(AppError::Io)?;
                }
                stack.push((entry_path, dest_path));
            } else {
                // CORRECTION 2 : On appelle explicitement tokio::fs::copy
                tokio::fs::copy(&entry_path, &dest_path)
                    .await
                    .map_err(AppError::Io)?;
            }
        }
    }

    Ok(())
}
// Alias pour écrire un fichier (utilise tokio)
pub async fn write(
    path: impl AsRef<std::path::Path>,
    contents: impl AsRef<[u8]>,
) -> crate::utils::Result<()> {
    tokio::fs::write(path, contents)
        .await
        .map_err(|e| crate::utils::AppError::System(anyhow::anyhow!(e)))
}

#[instrument(skip(path), fields(path = ?path))]
pub async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    debug!("Lecture du fichier JSON");

    if !exists(path).await {
        return Err(AppError::NotFound(format!(
            "Fichier introuvable : {:?}",
            path
        )));
    }

    // Lecture brute via Tokio
    // CORRECTION ICI : On passe directement le constructeur AppError::Io
    let content = fs::read_to_string(path).await.map_err(AppError::Io)?;

    // Parsing via notre utilitaire centralisé (qui gère les erreurs JSON)
    json::parse(&content)
}

// --- ÉCRITURE ATOMIQUE ---
#[instrument(skip(content, path), fields(path = ?path))]
pub async fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    debug!("Début écriture atomique (Raw)");

    // 1. Gestion du dossier parent
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }

    // 2. Fichier temporaire
    let tmp_path = path.with_extension("tmp");

    let mut file = fs::File::create(&tmp_path).await.map_err(AppError::Io)?;

    // Écriture
    file.write_all(content).await.map_err(AppError::Io)?;

    // Flush + Sync (Garantie durabilité matérielle)
    file.flush().await.map_err(AppError::Io)?;
    file.sync_all().await.map_err(AppError::Io)?;

    // 3. Renommage atomique
    if let Err(e) = fs::rename(&tmp_path, path).await {
        error!(
            "Échec du renommage atomique de {:?} vers {:?}",
            tmp_path, path
        );
        let _ = remove_file(&tmp_path).await;
        return Err(AppError::Io(e));
    }

    debug!("Succès écriture atomique : {:?}", path);
    Ok(())
}

/// Wrapper pour l'écriture atomique d'objets JSON
#[instrument(skip(data, path), fields(path = ?path))]
pub async fn write_json_atomic<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    debug!("Sérialisation JSON pour écriture atomique");
    // Sérialisation
    let content = json::stringify_pretty(data)?;
    // Délégation à la fonction générique
    write_atomic(path, content.as_bytes()).await
}

// --- UTILITAIRES FICHIERS ---

/// Vérifie qu'un fichier ou dossier existe (Async)
pub async fn exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

/// Crée un dossier et ses parents s'ils n'existent pas (mkdir -p)
pub async fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        debug!("Création du dossier : {:?}", path);
        fs::create_dir_all(path).await.map_err(AppError::Io)?;
    }
    Ok(())
}

/// Supprime un fichier proprement (sans erreur si inexistant)
pub async fn remove_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).await.map_err(AppError::Io)?;
    }
    Ok(())
}

/// Supprime un dossier récursivement
pub async fn remove_dir_all(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).await.map_err(AppError::Io)?;
    }
    Ok(())
}
// Lecture brute (String)
pub async fn read_to_string(path: &Path) -> Result<String> {
    if !exists(path).await {
        return Err(AppError::NotFound(format!(
            "Fichier introuvable : {:?}",
            path
        )));
    }
    fs::read_to_string(path).await.map_err(AppError::Io)
}

// Renommage (Move)
pub async fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).await.map_err(AppError::Io)
}

/// Liste le contenu d'un dossier (Async)
pub async fn read_dir(path: &Path) -> Result<ReadDir> {
    // On délègue à Tokio et on convertit l'erreur si besoin
    fs::read_dir(path).await.map_err(AppError::Io)
}

#[derive(Clone, Debug)]
pub struct ProjectScope {
    root: std::path::PathBuf,
}

impl ProjectScope {
    /// Initialise un scope.
    /// Vérifie que la racine existe et la verrouille sous forme canonique (absolue).
    pub fn new(root: impl Into<std::path::PathBuf>) -> crate::utils::Result<Self> {
        let root = root.into();
        // On s'assure que la racine existe pour pouvoir la canonicaliser
        if !root.exists() {
            std::fs::create_dir_all(&root).map_err(crate::utils::AppError::Io)?;
        }
        // Canonicalize résout les symlinks et les ".." pour donner le vrai chemin physique
        let canonical = root.canonicalize().map_err(crate::utils::AppError::Io)?;
        Ok(Self { root: canonical })
    }

    /// Écrit un fichier de manière sécurisée.
    /// Bloque toute tentative de sortir du scope (ex: "../hack.txt").
    pub async fn write(
        &self,
        relative_path: impl AsRef<std::path::Path>,
        content: &[u8],
    ) -> crate::utils::Result<()> {
        let target_path = self.root.join(relative_path);

        // 1. Protection contre les injections de chemins absolus
        if target_path.is_absolute() && !target_path.starts_with(&self.root) {
            return Err(crate::utils::AppError::System(anyhow::anyhow!(
                "Security Violation: Chemin absolu hors scope interdit : {:?}",
                target_path
            )));
        }

        // 2. Gestion du dossier parent
        let parent = target_path.parent().ok_or_else(|| {
            crate::utils::AppError::System(anyhow::anyhow!("Fichier sans dossier parent"))
        })?;

        // On crée le dossier parent s'il n'existe pas (pour pouvoir le canonicaliser ensuite)
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(crate::utils::AppError::Io)?;
        }

        // 3. Le Juge de Paix : Canonicalisation
        // C'est la seule façon sûre de vérifier qu'on n'est pas remonté via ".."
        let canonical_parent = parent.canonicalize().map_err(crate::utils::AppError::Io)?;

        if !canonical_parent.starts_with(&self.root) {
            return Err(crate::utils::AppError::System(anyhow::anyhow!(
                "Security Violation: Tentative d'écriture hors du scope ({:?} est hors de {:?})",
                canonical_parent,
                self.root
            )));
        }

        // 4. Écriture atomique (via notre primitive existante)
        // On reconstruit le chemin sûr avec le nom de fichier final
        let safe_filename = target_path.file_name().unwrap();
        let safe_path = canonical_parent.join(safe_filename);

        write_atomic(&safe_path, content).await
    }
}

#[instrument(skip(content, path), fields(path = ?path))]
pub async fn write_compressed_atomic(path: &Path, content: &[u8]) -> Result<()> {
    debug!("Compression et écriture atomique");
    let data = content.to_vec();
    let compressed = tokio::task::spawn_blocking(move || {
        super::compression::compress(&data) // Utilise la fondation interne
    })
    .await
    .map_err(|e| AppError::System(anyhow::anyhow!(e)))??;

    write_atomic(path, &compressed).await
}

/// Lit un fichier compressé avec Zstd.
#[instrument(skip(path), fields(path = ?path))]
pub async fn read_compressed(path: &Path) -> Result<Vec<u8>> {
    debug!("Lecture et décompression asynchrone");

    // 1. Lecture physique asynchrone du fichier
    let compressed_data = fs::read(path).await.map_err(AppError::Io)?;

    // 2. Décompression CPU-bound déportée
    // On utilise notre primitive 'super::compression::decompress'
    tokio::task::spawn_blocking(move || super::compression::decompress(&compressed_data))
        .await
        // Gestion du JoinError (si le thread spawné panique)
        .map_err(|e| AppError::System(anyhow::anyhow!("Échec du thread de décompression: {}", e)))?
}

/// Écrit un objet JSON compressé de manière atomique.
pub async fn write_json_compressed_atomic<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let content = crate::utils::json::stringify(data)?; // On utilise stringify compact pour le stockage
    write_compressed_atomic(path, content.as_bytes()).await
}

/// Lit un objet JSON compressé.
pub async fn read_json_compressed<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let decompressed = read_compressed(path).await?;
    let content = String::from_utf8(decompressed)
        .map_err(|e| AppError::System(anyhow::anyhow!("UTF-8 Error: {}", e)))?;

    crate::utils::json::parse(&content)
}
// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::core::{DateTime, Utc, Uuid};
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct MockDocument {
        uid: Uuid,
        content: String,
        timestamp: DateTime<Utc>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
    }
    #[tokio::test]
    async fn test_write_atomic_raw() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("raw.txt");
        let content = b"Hello from atomic write";

        write_atomic(&file_path, content)
            .await
            .expect("Write failed");

        assert!(file_path.exists());
        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, "Hello from atomic write");
    }

    #[tokio::test]
    async fn test_write_json_atomic() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.json");

        let original_data = TestData {
            id: 42,
            name: "Raise Architecture".to_string(),
        };

        write_json_atomic(&file_path, &original_data)
            .await
            .expect("JSON Write failed");

        assert!(file_path.exists());

        let read_data: TestData = read_json(&file_path).await.expect("Read failed");
        assert_eq!(original_data, read_data);
    }

    #[tokio::test]
    async fn test_atomic_write_and_read() {
        // Setup environnement temporaire
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_data.json");

        let original_data = TestData {
            id: 42,
            name: "Raise Architecture".to_string(),
        };

        // 1. Test Écriture Atomique
        write_json_atomic(&file_path, &original_data)
            .await
            .expect("L'écriture atomique a échoué");

        assert!(file_path.exists(), "Le fichier final doit exister");
        assert!(
            !file_path.with_extension("tmp").exists(),
            "Le fichier tmp doit avoir disparu"
        );

        // 2. Test Lecture
        let read_data: TestData = read_json(&file_path).await.expect("La lecture a échoué");

        assert_eq!(
            original_data, read_data,
            "Les données lues doivent correspondre"
        );
    }

    #[tokio::test]
    async fn test_read_missing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("ghost.json");

        // Doit retourner une erreur
        let res: Result<TestData> = read_json(&file_path).await;

        assert!(res.is_err());
        match res.unwrap_err() {
            AppError::NotFound(_) => assert!(true), // C'est ce qu'on attend
            _ => panic!("Devrait retourner une AppError::NotFound"),
        }
    }

    #[tokio::test]
    async fn test_ensure_dir_creation() {
        let dir = tempdir().unwrap();
        let deep_path = dir.path().join("a").join("b").join("c");

        ensure_dir(&deep_path).await.expect("ensure_dir failed");
        assert!(deep_path.exists());
        assert!(deep_path.is_dir());
    }
    #[tokio::test]
    async fn test_project_scope_security() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("safe_zone");
        let scope = ProjectScope::new(&root).unwrap();

        // 1. Écriture légitime
        let res_ok = scope.write("doc.txt", b"safe").await;
        assert!(res_ok.is_ok());
        assert!(root.join("doc.txt").exists());

        // 2. Tentative d'évasion simple (..)
        let res_hack = scope.write("../hack.txt", b"danger").await;
        assert!(
            res_hack.is_err(),
            "Devrait bloquer la sortie du dossier via .."
        );

        // 3. Tentative d'évasion complexe (dossier imbriqué + retour)
        // safe_zone/a/b/../../../hack.txt -> safe_zone/../hack.txt -> SORTIE
        let res_deep_hack = scope.write("a/b/../../../hack_deep.txt", b"danger").await;
        assert!(res_deep_hack.is_err(), "Devrait bloquer l'évasion complexe");
    }

    #[tokio::test]
    async fn test_compressed_binary_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.bin.zstd");
        let original_data = b"Donnees binaires brutes pour test de persistance";

        // 1. Écriture
        write_compressed_atomic(&file_path, original_data)
            .await
            .expect("Échec écriture");

        // 2. Vérification que le fichier est bien compressé (différent de l'original)
        let raw_on_disk = tokio::fs::read(&file_path).await.unwrap();
        assert_ne!(raw_on_disk, original_data);

        // 3. Lecture et décompression
        let restored_data = read_compressed(&file_path).await.expect("Échec lecture");
        assert_eq!(restored_data, original_data);
    }

    /// Test du cycle complet : Écriture JSON -> Lecture JSON (Compressé)
    #[tokio::test]
    async fn test_compressed_json_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("doc.json.zstd");

        let original_doc = MockDocument {
            uid: Uuid::new_v4(),
            content: "Contenu hautement compressible...".repeat(10),
            timestamp: Utc::now(),
        };

        // 1. Écriture JSON compressée
        write_json_compressed_atomic(&file_path, &original_doc)
            .await
            .expect("Échec écriture JSON");

        // 2. Lecture et désérialisation
        let restored_doc: MockDocument = read_json_compressed(&file_path)
            .await
            .expect("Échec lecture JSON");

        assert_eq!(original_doc, restored_doc);
    }

    /// Vérification de l'erreur en cas de fichier corrompu
    #[tokio::test]
    async fn test_read_compressed_corrupted_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("corrupt.zstd");

        // Écrire des données qui ne sont pas du Zstd
        tokio::fs::write(&file_path, b"not zstd data")
            .await
            .unwrap();

        let result = read_compressed(&file_path).await;

        // On s'attend à une erreur système ou IO de décompression
        assert!(result.is_err());
    }
}
