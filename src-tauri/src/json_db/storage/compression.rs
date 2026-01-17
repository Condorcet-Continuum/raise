// FICHIER : src-tauri/src/json_db/storage/compression.rs

//! Module de compression utilisant Zstd pour optimiser l'espace disque.

use std::io::{Read, Write};

/// Compresse des données brutes (octets) en utilisant Zstd.
pub fn compress(data: &[u8]) -> Vec<u8> {
    // Niveau 3 est le compromis standard de Zstd (rapide et efficace)
    let mut encoder = zstd::Encoder::new(Vec::new(), 3).expect("Failed to init zstd encoder");
    encoder
        .write_all(data)
        .expect("Failed to write to zstd encoder");
    encoder.finish().expect("Failed to finish zstd compression")
}

/// Décompresse des données Zstd pour retrouver les données originales.
pub fn decompress(data: &[u8]) -> Vec<u8> {
    let mut decoder = zstd::Decoder::new(data).expect("Failed to init zstd decoder");
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("Failed to decompress zstd data");
    decompressed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_roundtrip() {
        let original = b"Ceci est un test de compression avec Zstd pour le projet RAISE.";

        // 1. Compression
        let compressed = compress(original);
        assert!(compressed.len() > 0);

        // 2. Décompression
        let restored = decompress(&compressed);

        // 3. Vérification de l'intégrité
        assert_eq!(original.to_vec(), restored);
    }

    #[test]
    fn test_compression_efficiency() {
        // Un texte répétitif se compresse très bien
        let original = "RAISE ".repeat(100);
        let original_bytes = original.as_bytes();

        let compressed = compress(original_bytes);

        println!(
            "Original: {} bytes, Compressed: {} bytes",
            original_bytes.len(),
            compressed.len()
        );
        assert!(compressed.len() < original_bytes.len());
    }
}
