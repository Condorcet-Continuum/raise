// FICHIER : src-tauri/src/utils/fs.rs

use crate::raise_error;
use crate::utils::{json, RaiseResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::fs;

// Nettoyage des imports inutilisés pour une propreté maximale
use tracing::instrument;

// --- RE-EXPORTS (Isolation de la couche OS) ---
pub use include_dir::{include_dir, Dir};
pub use std::path::{Component, Path, PathBuf};
pub use walkdir::WalkDir;

// --- LECTURE & ASYNC I/O ---
pub use tempfile::{tempdir, TempDir};
pub use tokio::fs::{DirEntry, File, ReadDir};
pub use tokio::io::AsyncWriteExt;

/// Crée récursivement un répertoire.
pub async fn create_dir_all(path: impl AsRef<std::path::Path>) -> RaiseResult<()> {
    let p = path.as_ref();
    if let Err(e) = tokio::fs::create_dir_all(p).await {
        raise_error!(
            "ERR_FS_CREATE_DIR",
            error = e,
            context = serde_json::json!({ "path": p.to_string_lossy() })
        );
    }
    Ok(())
}

/// Copie récursivement un dossier complet.
pub async fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> RaiseResult<()> {
    let src = src.as_ref().to_path_buf();
    let dst = dst.as_ref().to_path_buf();

    if !tokio::fs::try_exists(&src).await.unwrap_or(false) {
        raise_error!(
            "ERR_FS_COPY_DIR_NOT_FOUND",
            error = "Le dossier source n'existe pas",
            context = serde_json::json!({ "src": src.to_string_lossy() })
        );
    }

    if !tokio::fs::try_exists(&dst).await.unwrap_or(false) {
        if let Err(e) = tokio::fs::create_dir_all(&dst).await {
            raise_error!(
                "ERR_FS_COPY_DIR_CREATE_DST",
                error = e,
                context = serde_json::json!({ "dst": dst.to_string_lossy() })
            );
        }
    }

    let mut stack = vec![(src, dst)];

    while let Some((current_src, current_dst)) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&current_src).await {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_FS_COPY_DIR_READ",
                error = e,
                context = serde_json::json!({ "current_src": current_src.to_string_lossy() })
            ),
        };

        while let Some(entry) = match entries.next_entry().await {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_FS_COPY_DIR_ENTRY",
                error = e,
                context = serde_json::json!({ "current_src": current_src.to_string_lossy() })
            ),
        } {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let dest_path = current_dst.join(entry_name);

            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => raise_error!(
                    "ERR_FS_COPY_DIR_FILETYPE",
                    error = e,
                    context = serde_json::json!({ "entry_path": entry_path.to_string_lossy() })
                ),
            };

            if file_type.is_dir() {
                if !tokio::fs::try_exists(&dest_path).await.unwrap_or(false) {
                    if let Err(e) = tokio::fs::create_dir_all(&dest_path).await {
                        raise_error!(
                            "ERR_FS_COPY_DIR_MKDIR",
                            error = e,
                            context =
                                serde_json::json!({ "dest_path": dest_path.to_string_lossy() })
                        );
                    }
                }
                stack.push((entry_path, dest_path));
            } else if let Err(e) = tokio::fs::copy(&entry_path, &dest_path).await {
                raise_error!(
                    "ERR_FS_COPY_DIR_COPY_FILE",
                    error = e,
                    context = serde_json::json!({
                        "from": entry_path.to_string_lossy(),
                        "to": dest_path.to_string_lossy()
                    })
                );
            }
        }
    }
    Ok(())
}

pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> RaiseResult<()> {
    let p = path.as_ref();
    if let Err(e) = tokio::fs::write(p, contents).await {
        raise_error!(
            "ERR_FS_WRITE_FILE",
            error = e,
            context = serde_json::json!({ "path": p.to_string_lossy() })
        );
    }
    Ok(())
}

#[instrument(skip(path), fields(path = ?path.as_ref()))]
pub async fn read(path: impl AsRef<Path>) -> RaiseResult<Vec<u8>> {
    let p = path.as_ref();
    match tokio::fs::read(p).await {
        Ok(data) => Ok(data),
        Err(e) => raise_error!(
            "ERR_FS_READ_FILE",
            error = e,
            context = serde_json::json!({ "path": p.to_string_lossy() })
        ),
    }
}

pub async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> RaiseResult<u64> {
    let from_path = from.as_ref();
    let to_path = to.as_ref();

    match tokio::fs::copy(from_path, to_path).await {
        Ok(size) => Ok(size),
        Err(e) => raise_error!(
            "ERR_FS_COPY_FILE",
            error = e,
            context = serde_json::json!({
                "from": from_path.to_string_lossy(),
                "to": to_path.to_string_lossy()
            })
        ),
    }
}

#[instrument(skip(path), fields(path = ?path))]
pub async fn read_json<T: DeserializeOwned>(path: &Path) -> RaiseResult<T> {
    if !exists(path).await {
        raise_error!(
            "ERR_FS_NOT_FOUND",
            error = "Fichier JSON introuvable",
            context = serde_json::json!({ "path": path.to_string_lossy() })
        );
    }
    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => raise_error!(
            "ERR_FS_READ_JSON",
            error = e,
            context = serde_json::json!({ "path": path.to_string_lossy() })
        ),
    };
    json::parse(&content)
}

// --- ÉCRITURE ATOMIQUE ---

#[instrument(skip(content, path), fields(path = ?path))]
pub async fn write_atomic(path: &Path, content: &[u8]) -> RaiseResult<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }

    let tmp_path = path.with_extension("tmp");
    let mut file = match fs::File::create(&tmp_path).await {
        Ok(f) => f,
        Err(e) => raise_error!(
            "ERR_FS_CREATE_TMP_FILE",
            error = e,
            context = serde_json::json!({ "tmp_path": tmp_path.to_string_lossy() })
        ),
    };

    if let Err(e) = file.write_all(content).await {
        raise_error!(
            "ERR_FS_WRITE_TMP",
            error = e,
            context = serde_json::json!({ "path": tmp_path.to_string_lossy() })
        );
    }
    file.flush().await.ok();
    file.sync_all().await.ok();

    if let Err(e) = fs::rename(&tmp_path, path).await {
        let _ = remove_file(&tmp_path).await;
        raise_error!(
            "ERR_FS_RENAME_ATOMIC",
            error = e,
            context = serde_json::json!({
                "tmp": tmp_path.to_string_lossy(),
                "final": path.to_string_lossy()
            })
        );
    }
    Ok(())
}

pub async fn write_json_atomic<T: Serialize>(path: &Path, data: &T) -> RaiseResult<()> {
    let content = json::stringify_pretty(data)?;
    write_atomic(path, content.as_bytes()).await
}

pub async fn exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

pub async fn ensure_dir(path: &Path) -> RaiseResult<()> {
    if !path.exists() {
        if let Err(e) = fs::create_dir_all(path).await {
            raise_error!(
                "ERR_FS_ENSURE_DIR",
                error = e,
                context = serde_json::json!({ "path": path.to_string_lossy() })
            );
        }
    }
    Ok(())
}

pub async fn remove_file(path: &Path) -> RaiseResult<()> {
    if path.exists() {
        if let Err(e) = fs::remove_file(path).await {
            raise_error!(
                "ERR_FS_REMOVE_FILE",
                error = e,
                context = serde_json::json!({ "path": path.to_string_lossy() })
            );
        }
    }
    Ok(())
}

pub async fn remove_dir_all(path: &Path) -> RaiseResult<()> {
    if path.exists() {
        if let Err(e) = fs::remove_dir_all(path).await {
            raise_error!(
                "ERR_FS_REMOVE_DIR",
                error = e,
                context = serde_json::json!({ "path": path.to_string_lossy() })
            );
        }
    }
    Ok(())
}

pub async fn read_to_string(path: &Path) -> RaiseResult<String> {
    match fs::read_to_string(path).await {
        Ok(s) => Ok(s),
        Err(e) => raise_error!(
            "ERR_FS_READ_STR",
            error = e,
            context = serde_json::json!({ "path": path.to_string_lossy() })
        ),
    }
}

pub async fn rename(from: &Path, to: &Path) -> RaiseResult<()> {
    if let Err(e) = fs::rename(from, to).await {
        raise_error!(
            "ERR_FS_RENAME",
            error = e,
            context =
                serde_json::json!({ "from": from.to_string_lossy(), "to": to.to_string_lossy() })
        );
    }
    Ok(())
}

pub async fn read_dir(path: &Path) -> RaiseResult<ReadDir> {
    match fs::read_dir(path).await {
        Ok(rd) => Ok(rd),
        Err(e) => raise_error!(
            "ERR_FS_READ_DIR",
            error = e,
            context = serde_json::json!({ "path": path.to_string_lossy() })
        ),
    }
}

// --- PROJECT SCOPE (Security Sandbox) ---

#[derive(Clone, Debug)]
pub struct ProjectScope {
    root: std::path::PathBuf,
}

impl ProjectScope {
    pub fn new(root: impl Into<std::path::PathBuf>) -> RaiseResult<Self> {
        let root = root.into();
        if !root.exists() {
            std::fs::create_dir_all(&root).ok();
        }
        let canonical = match root.canonicalize() {
            Ok(path) => path,
            Err(e) => raise_error!(
                "ERR_FS_SCOPE_INIT",
                context = serde_json::json!({
                    "root_provided": root.to_string_lossy(),
                    "io_error": e.to_string(),
                    "action": "initialize_fs_scope",
                    "hint": "Le chemin racine est introuvable ou inaccessible. Vérifiez les permissions et l'existence du dossier."
                })
            ),
        };
        Ok(Self { root: canonical })
    }

    pub async fn write(&self, relative_path: impl AsRef<Path>, content: &[u8]) -> RaiseResult<()> {
        let rel = relative_path.as_ref();

        // 1. Interdiction stricte des composants de remontée (..)
        // Cela empêche l'évasion avant même de toucher au disque
        if rel.components().any(|c| matches!(c, Component::ParentDir)) {
            raise_error!(
                "ERR_FS_SECURITY_VIOLATION",
                error = "Évasion de scope détectée (tentative de remontée via '..')",
                context = serde_json::json!({ "path": rel.to_string_lossy() })
            );
        }

        // 2. Construction du chemin final
        let target_path = self.root.join(rel);

        // 3. Double vérification : le chemin doit toujours être sous la racine
        // (Utile si relative_path était un chemin absolu)
        if !target_path.starts_with(&self.root) {
            raise_error!(
                "ERR_FS_SECURITY_VIOLATION",
                error = "Évasion de scope détectée (chemin hors limite)",
                context = serde_json::json!({ "path": rel.to_string_lossy() })
            );
        }

        if let Some(parent) = target_path.parent() {
            ensure_dir(parent).await?;
        }
        write_atomic(&target_path, content).await
    }
}

// --- OPERATIONS COMPRESSÉES ---

pub async fn write_compressed_atomic(path: &Path, content: &[u8]) -> RaiseResult<()> {
    let data = content.to_vec();
    let join_res = tokio::task::spawn_blocking(move || super::compression::compress(&data)).await;
    let compressed = match join_res {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(e),
        Err(e) => raise_error!("ERR_FS_COMPRESS_PANIC", error = e),
    };
    write_atomic(path, &compressed).await
}

pub async fn read_compressed(path: &Path) -> RaiseResult<Vec<u8>> {
    let compressed_data = read(path).await?;
    let join_res =
        tokio::task::spawn_blocking(move || super::compression::decompress(&compressed_data)).await;
    match join_res {
        Ok(Ok(d)) => Ok(d),
        Ok(Err(e)) => Err(e),
        Err(e) => raise_error!("ERR_FS_DECOMPRESS_PANIC", error = e),
    }
}

pub async fn write_json_compressed_atomic<T: Serialize>(path: &Path, data: &T) -> RaiseResult<()> {
    let content = json::stringify(data)?;
    write_compressed_atomic(path, content.as_bytes()).await
}

pub async fn read_json_compressed<T: DeserializeOwned>(path: &Path) -> RaiseResult<T> {
    let decompressed = read_compressed(path).await?;
    let content = match String::from_utf8(decompressed) {
        Ok(s) => s,
        Err(e) => {
            // On extrait l'erreur UTF-8 sous-jacente pour obtenir l'index précis
            let utf8_err = e.utf8_error();
            raise_error!(
                "ERR_DATA_CORRUPTION_UTF8",
                context = serde_json::json!({
                    "action": "decompress_and_decode",
                    "error_type": "invalid_utf8_sequence",
                    "valid_up_to": utf8_err.valid_up_to(),
                    "error_details": e.to_string(),
                    "hint": "Les données ne sont pas au format UTF-8 valide. Vérifiez l'intégrité du fichier source."
                })
            )
        }
    };
    json::parse(&content)
}

pub async fn write_bincode_compressed_atomic<T: Serialize>(
    path: &Path,
    data: &T,
) -> RaiseResult<()> {
    let binary_data = json::to_binary(data)?;
    write_compressed_atomic(path, &binary_data).await
}

pub async fn read_bincode_compressed<T: DeserializeOwned>(path: &Path) -> RaiseResult<T> {
    let decompressed_binary = read_compressed(path).await?;
    json::from_binary(&decompressed_binary)
}

// =========================================================================
// TESTS UNITAIRES (RAISE standard)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
    }

    #[tokio::test]
    async fn test_atomic_write_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("atomic.json");
        let data = TestData {
            id: 1,
            name: "Raise".into(),
        };

        write_json_atomic(&file_path, &data).await.unwrap();
        let restored: TestData = read_json(&file_path).await.unwrap();
        assert_eq!(data, restored);
    }

    #[tokio::test]
    async fn test_compression_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("comp.zstd");
        let original = b"Donnees hautement compressibles...".repeat(10);

        write_compressed_atomic(&file_path, &original)
            .await
            .unwrap();
        let restored = read_compressed(&file_path).await.unwrap();
        assert_eq!(original, restored);
    }

    #[tokio::test]
    async fn test_project_scope_isolation() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("sandbox");
        let scope = ProjectScope::new(&root).expect("Scope init HS");

        assert!(scope.write("test.txt", b"ok").await.is_ok());
        let res = scope.write("../secret.txt", b"hack").await;
        assert!(res.is_err(), "Le scope a laisse passer une evasion");
    }
}
