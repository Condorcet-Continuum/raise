// FICHIER : src-tauri/src/utils/io/compression.rs

use crate::raise_error; // Celle-ci devrait fonctionner si mod.rs est bien configuré
use crate::utils::core::error::RaiseResult;
use crate::utils::data::json::json_value;
use crate::utils::io::{CompressionDecoder, CompressionEncoder};

use super::io_traits::{SyncRead, SyncWrite};

/// Compresse un buffer d'octets en utilisant l'algorithme Zstd (niveau 3).
/// Idéal pour les documents JSON et les snapshots de base de données.
pub fn compress(data: &[u8]) -> RaiseResult<Vec<u8>> {
    let data_len = data.len();

    // 1. Initialisation de l'encodeur (Niveau 3 = équilibre parfait)
    let mut encoder = match CompressionEncoder::new(Vec::new(), 3) {
        Ok(enc) => enc,
        Err(e) => raise_error!(
            "ERR_COMPRESS_INIT",
            error = e,
            context = json_value!({ "input_size": data_len }) // 🎯 json_value!
        ),
    };

    // 2. Écriture des données dans le flux de compression
    if let Err(e) = encoder.write_all(data) {
        raise_error!(
            "ERR_COMPRESS_WRITE",
            error = e,
            context = json_value!({ "input_size": data_len })
        );
    }

    // 3. Finalisation du flux
    match encoder.finish() {
        Ok(res) => {
            tracing::trace!("Compression Zstd : {} -> {} octets", data_len, res.len());
            Ok(res)
        }
        Err(e) => raise_error!(
            "ERR_COMPRESS_FINISH",
            error = e,
            context = json_value!({ "input_size": data_len })
        ),
    }
}

/// Décompresse un buffer compressé avec Zstd.
pub fn decompress(data: &[u8]) -> RaiseResult<Vec<u8>> {
    let compressed_len = data.len();

    let mut decoder = match CompressionDecoder::new(data) {
        Ok(dec) => dec,
        Err(e) => raise_error!(
            "ERR_DECOMPRESS_INIT",
            error = e,
            context = json_value!({ "compressed_size": compressed_len })
        ),
    };

    let mut decompressed = Vec::new();
    if let Err(e) = decoder.read_to_end(&mut decompressed) {
        raise_error!(
            "ERR_DECOMPRESS_READ",
            error = e,
            context = json_value!({ "compressed_size": compressed_len })
        );
    }

    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::core::error::AppError;

    #[test]
    fn test_compression_roundtrip() {
        let original = b"Donnees de test RAISE pour verifier l'integrite de la compression.";
        let compressed = compress(original).expect("Compression HS");
        let restored = decompress(&compressed).expect("Decompression HS");
        assert_eq!(original.to_vec(), restored);
    }

    #[test]
    fn test_compression_efficiency() {
        let original = "RAISE".repeat(100);
        let compressed = compress(original.as_bytes()).unwrap();
        assert!(compressed.len() < original.len());
    }

    #[test]
    fn test_decompress_invalid_data() {
        let result = decompress(b"pas du zstd");
        assert!(result.is_err());

        let AppError::Structured(data) = result.unwrap_err();
        assert_eq!(data.code, "ERR_DECOMPRESS_READ");
    }
}
