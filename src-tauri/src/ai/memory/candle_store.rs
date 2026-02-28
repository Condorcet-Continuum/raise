use super::{MemoryRecord, VectorStore};
use crate::utils::prelude::*;
use crate::utils::{
    async_trait,
    io::{self, PathBuf},
    AsyncRwLock, HashMap, Ordering,
};
use candle_core::{Device, Tensor};

/// Store vectoriel local RAISE avec filtrage sémantique et persistance Zstd.
pub struct CandleLocalStore {
    storage_path: PathBuf,
    device: Device,
    state: AsyncRwLock<StoreState>,
}

#[derive(Default)]
struct StoreState {
    records: Vec<MemoryRecord>,
    vector_matrix: Option<Tensor>,
}

impl CandleLocalStore {
    pub fn new(path: &Path, device: &Device) -> Self {
        Self {
            storage_path: path.to_path_buf(),
            device: device.clone(),
            state: AsyncRwLock::new(StoreState::default()),
        }
    }

    /// Reconstruit la matrice de recherche à partir des records.
    fn compute_matrix(records: &[MemoryRecord], device: &Device) -> RaiseResult<Option<Tensor>> {
        let valid_vectors: Vec<&Vec<f32>> =
            records.iter().filter_map(|r| r.vectors.as_ref()).collect();

        if valid_vectors.is_empty() {
            return Ok(None);
        }

        let n = valid_vectors.len();
        let d = valid_vectors[0].len();
        let flat_vecs: Vec<f32> = valid_vectors.into_iter().flatten().cloned().collect();

        let matrix = match Tensor::from_vec(flat_vecs, (n, d), device) {
            Ok(m) => m,
            Err(e) => {
                // La macro interrompt la fonction et renvoie une erreur typée RAISE
                raise_error!(
                    "ERR_VECTOR_MATRIX_CREATION_FAILED",
                    error = e,
                    context = json!({
                        "action": "build_search_index",
                        "vectors_count": n,
                        "dimension": d,
                        "device": format!("{:?}", device),
                        "hint": "Échec de la création de la matrice de recherche. Vérifiez la cohérence des dimensions ou la mémoire GPU disponible."
                    })
                )
            }
        };

        Ok(Some(matrix))
    }

    /// Sauvegarde asynchrone et atomique (Records + Vecteurs).
    pub async fn save(&self) -> RaiseResult<()> {
        let state = self.state.read().await;
        let json_path = self.storage_path.join("memory_records.json.zstd");

        // Utilisation de la façade io pour l'écriture atomique compressée
        io::write_json_compressed_atomic(&json_path, &state.records).await?;

        if let Some(ref matrix) = state.vector_matrix {
            let tensor_path = self.storage_path.join("memory_vectors.safetensors");
            // 1. On clone pour le thread, on garde l'original pour le diagnostic
            let thread_path = tensor_path.clone();
            let matrix_clone = matrix.clone();

            let join_handle = tokio::task::spawn_blocking(move || {
                let mut map = std::collections::HashMap::new();
                map.insert("vectors".to_string(), matrix_clone);
                // On utilise le clone ici
                candle_core::safetensors::save(&map, thread_path)
            })
            .await;

            // 2. Gestion de l'erreur du Thread (JoinError)
            let save_result = match join_handle {
                Ok(res) => res,
                Err(e) => {
                    raise_error!(
                        "ERR_THREAD_PANIC_DURING_SAVE",
                        error = e,
                        context = json!({ "action": "spawn_blocking_safetensors" })
                    )
                }
            };

            // 3. Gestion de l'erreur Safetensors/IO
            if let Err(e) = save_result {
                raise_error!(
                    "ERR_AI_SAFE_TENSORS_SAVE_FAILED",
                    error = e,
                    context = json!({
                        "action": "persist_vector_memory",
                        "path": tensor_path.to_string_lossy(), // L'original est encore là !
                        "hint": "Vérifiez que le disque n'est pas plein."
                    })
                )
            }
        }
        Ok(())
    }

    /// Chargement asynchrone.
    pub async fn load(&self) -> RaiseResult<()> {
        let json_path = self.storage_path.join("memory_records.json.zstd");
        if !io::exists(&json_path).await {
            return Ok(());
        }

        let loaded_records: Vec<MemoryRecord> = io::read_json_compressed(&json_path).await?;
        let matrix = Self::compute_matrix(&loaded_records, &self.device)?;

        let mut state = self.state.write().await;
        state.records = loaded_records;
        state.vector_matrix = matrix;
        Ok(())
    }
}

#[async_trait]
impl VectorStore for CandleLocalStore {
    async fn init_collection(&self, _col: &str, _size: u64) -> RaiseResult<()> {
        io::ensure_dir(&self.storage_path).await
    }

    async fn add_documents(&self, _col: &str, mut records: Vec<MemoryRecord>) -> RaiseResult<()> {
        let mut state = self.state.write().await;
        state.records.append(&mut records);
        state.vector_matrix = Self::compute_matrix(&state.records, &self.device)?;
        Ok(())
    }

    async fn search_similarity(
        &self,
        _col: &str,
        query_vec: &[f32],
        limit: u64,
        threshold: f32,
        filter: Option<HashMap<String, String>>, // Utilise utils::HashMap
    ) -> RaiseResult<Vec<MemoryRecord>> {
        let state = self.state.read().await;
        let matrix = match &state.vector_matrix {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        // 1. Calcul des scores via Candle (Produit Matriciel)
        // 1. Préparation du vecteur de requête
        let q = match Tensor::from_slice(query_vec, (1, query_vec.len()), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_QUERY_INIT_FAILED", error = e),
        };

        // 2. Transposition ISOLÉE pour garantir le type Tensor
        let q_transposed = match q.t() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_TRANSPOSE_FAILED", error = e),
        };

        // 3. Multiplication matricielle avec le Tensor extrait
        let scores_tensor = match matrix.matmul(&q_transposed) {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_VECTOR_MATMUL_FAILED",
                error = e,
                context = json!({ "matrix_shape": format!("{:?}", matrix.shape()) })
            ),
        };

        // 4. Extraction finale
        let scores = match scores_tensor.flatten_all() {
            Ok(s) => match s.to_vec1::<f32>() {
                Ok(v) => v,
                Err(e) => raise_error!("ERR_VECTOR_DTYPE_CONVERSION", error = e),
            },
            Err(e) => raise_error!("ERR_VECTOR_FLATTEN_FAILED", error = e),
        };

        // 2. Application du filtre Métadonnées + Threshold
        // 2. Application du filtre Métadonnées + Threshold
        let mut ranked: Vec<(f32, usize)> = scores
            .into_iter()
            .enumerate()
            .filter(|(idx, score)| {
                if *score < threshold {
                    return false;
                }

                if let Some(ref f_map) = filter {
                    let record = &state.records[*idx];
                    for (k, v) in f_map {
                        let meta_match = record
                            .metadata
                            .get(k)
                            .map(|val| {
                                // Zéro allocation : On compare directement les valeurs
                                if let Some(s) = val.as_str() {
                                    s == v
                                } else if let Some(n) = val.as_i64() {
                                    v.parse::<i64>().is_ok_and(|parsed| parsed == n)
                                } else if let Some(b) = val.as_bool() {
                                    v.parse::<bool>().is_ok_and(|parsed| parsed == b)
                                } else {
                                    false
                                }
                            })
                            .unwrap_or(false);
                        if !meta_match {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|(i, s)| (s, i))
            .collect();

        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        Ok(ranked
            .into_iter()
            .take(limit as usize)
            .map(|(_, idx)| state.records[idx].clone())
            .collect())
    }
}

// --- TESTS UNITAIRES HYPER ROBUSTES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_search_with_metadata_filter() {
        let dir = io::tempdir().unwrap();
        let store = CandleLocalStore::new(dir.path(), &Device::Cpu);

        let recs = vec![
            MemoryRecord {
                id: "1".into(),
                content: "Doc Hardware".into(),
                metadata: json!({"category": "hardware", "priority": "high"}),
                vectors: Some(vec![1.0, 0.0]),
            },
            MemoryRecord {
                id: "2".into(),
                content: "Doc Software".into(),
                metadata: json!({"category": "software"}),
                vectors: Some(vec![0.9, 0.1]),
            },
        ];
        store.add_documents("tech", recs).await.unwrap();

        // Test 1: Recherche globale
        let res_all = store
            .search_similarity("tech", &[1.0, 0.0], 10, 0.0, None)
            .await
            .unwrap();
        assert_eq!(res_all.len(), 2);

        // Test 2: Recherche avec filtre Metadata
        let mut filter = HashMap::new();
        filter.insert("category".into(), "hardware".into());

        let res_filter = store
            .search_similarity("tech", &[1.0, 0.0], 10, 0.0, Some(filter))
            .await
            .unwrap();
        assert_eq!(res_filter.len(), 1);
        assert_eq!(res_filter[0].id, "1");
    }

    #[tokio::test]
    async fn test_persistence_integrity() {
        let dir = io::tempdir().unwrap();
        let path = dir.path();

        {
            let store = CandleLocalStore::new(path, &Device::Cpu);
            let rec = MemoryRecord {
                id: "P1".into(),
                content: "Persist".into(),
                metadata: json!({"status": "saved"}),
                // CORRECTION : Utilisation d'un vecteur normalisé (Norme L2 = 1.0)
                vectors: Some(vec![1.0, 0.0]),
            };
            store.add_documents("p", vec![rec]).await.unwrap();
            store.save().await.unwrap();
        }

        let new_store = CandleLocalStore::new(path, &Device::Cpu);
        new_store.load().await.unwrap();

        // Produit scalaire de [1.0, 0.0] avec lui-même = 1.0 (qui est bien > 0.9)
        let res = new_store
            .search_similarity("p", &[1.0, 0.0], 1, 0.9, None)
            .await
            .unwrap();

        // ASSERTION DE SÉCURITÉ : Vérifier qu'on a bien un résultat avant de lire l'index 0
        assert_eq!(
            res.len(),
            1,
            "La recherche devrait renvoyer exactement 1 résultat"
        );
        assert_eq!(res[0].metadata["status"], "saved");
    }
}
