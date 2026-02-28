// FICHIER : src-tauri/src/utils/compression.rs

use crate::raise_error; // Import explicite de la macro de fondation
use crate::utils::prelude::*;
use std::io::{Read, Write};

/// Compresse un buffer d'octets en utilisant l'algorithme Zstd (niveau 3).
/// Idéal pour les documents JSON et les snapshots de base de données.
pub fn compress(data: &[u8]) -> RaiseResult<Vec<u8>> {
    let data_len = data.len();

    // 1. Initialisation de l'encodeur
    // On utilise le niveau 3 (équilibre parfait performance/ratio)
    let mut encoder = match zstd::Encoder::new(Vec::new(), 3) {
        Ok(enc) => enc,
        Err(e) => raise_error!(
            "ERR_COMPRESS_INIT",
            error = e,
            context = json!({ "input_size": data_len })
        ),
    };

    // 2. Écriture des données dans le flux de compression
    if let Err(e) = encoder.write_all(data) {
        raise_error!(
            "ERR_COMPRESS_WRITE",
            error = e,
            context = json!({ "input_size": data_len })
        );
    }

    // 3. Finalisation du flux et récupération du Vec final
    match encoder.finish() {
        Ok(res) => {
            tracing::trace!("Compression Zstd : {} -> {} octets", data_len, res.len());
            Ok(res)
        }
        Err(e) => raise_error!(
            "ERR_COMPRESS_FINISH",
            error = e,
            context = json!({ "input_size": data_len })
        ),
    }
}

/// Décompresse un buffer compressé avec Zstd.
pub fn decompress(data: &[u8]) -> RaiseResult<Vec<u8>> {
    let compressed_len = data.len();

    // 1. Initialisation du décodeur
    let mut decoder = match zstd::Decoder::new(data) {
        Ok(dec) => dec,
        Err(e) => raise_error!(
            "ERR_DECOMPRESS_INIT",
            error = e,
            context = json!({ "compressed_size": compressed_len })
        ),
    };

    // 2. Lecture intégrale et décompression
    let mut decompressed = Vec::new();
    if let Err(e) = decoder.read_to_end(&mut decompressed) {
        raise_error!(
            "ERR_DECOMPRESS_READ",
            error = e,
            context = json!({ "compressed_size": compressed_len })
        );
    }

    Ok(decompressed)
}

// --- TESTS UNITAIRES (Standard RAISE) ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_roundtrip() {
        let original = b"Donnees de test RAISE pour verifier l'integrite de la compression.";

        let compressed = compress(original).expect("Compression HS");
        let restored = decompress(&compressed).expect("Decompression HS");

        assert_eq!(
            original.to_vec(),
            restored,
            "Donnees corrompues apres roundtrip"
        );
    }

    #[test]
    fn test_compression_efficiency() {
        // Un texte hautement répétitif doit être massivement réduit
        let original = "RAISE".repeat(100);
        let compressed = compress(original.as_bytes()).unwrap();

        assert!(
            compressed.len() < original.len(),
            "Zstd n'a pas reduit la taille"
        );
    }

    #[test]
    fn test_decompress_invalid_data() {
        let result = decompress(b"pas du zstd");

        assert!(result.is_err());

        if let Err(crate::utils::error::AppError::Structured(data)) = result {
            // Changement ici : Zstd échoue lors de la lecture effective
            assert_eq!(data.code, "ERR_DECOMPRESS_READ");
            assert_eq!(data.component, "COMPRESSION");
            assert!(!data.message.is_empty());
        } else {
            panic!("L'erreur devrait être de type AppError::Structured");
        }
    }
}
