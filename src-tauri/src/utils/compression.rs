use crate::utils::prelude::*;
use std::io::{Read, Write};

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = zstd::Encoder::new(Vec::new(), 3).map_err(AppError::Io)?;
    encoder.write_all(data).map_err(AppError::Io)?;
    let res = encoder.finish().map_err(AppError::Io)?;
    Ok(res)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = zstd::Decoder::new(data).map_err(AppError::Io)?;
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(AppError::Io)?;
    Ok(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_roundtrip() {
        // Données de test représentatives d'un petit document JSON
        let original =
            b"{\"id\": \"test-123\", \"status\": \"active\", \"msg\": \"RAISE foundation layer\"}";

        // 1. Test de compression
        let compressed = compress(original).expect("La compression a échoué");
        assert!(
            !compressed.is_empty(),
            "Le buffer compressé ne doit pas être vide"
        );

        // 2. Test de décompression
        let restored = decompress(&compressed).expect("La décompression a échoué");

        // 3. Vérification de l'intégrité totale
        assert_eq!(
            original.to_vec(),
            restored,
            "Les données restaurées doivent être identiques à l'original"
        );
    }

    #[test]
    fn test_compression_efficiency() {
        // Un texte avec beaucoup de répétitions pour maximiser l'efficacité de Zstd
        let original_text = "RAISE ".repeat(200);
        let original_bytes = original_text.as_bytes();

        let compressed = compress(original_bytes).expect("Compression failed");

        // Logging via la fondation
        debug!(
            "Efficacité Zstd : Original = {} octets, Compressé = {} octets (Ratio: {:.2}x)",
            original_bytes.len(),
            compressed.len(),
            original_bytes.len() as f32 / compressed.len() as f32
        );

        // Pour des données répétitives, Zstd doit être extrêmement performant
        assert!(
            compressed.len() < original_bytes.len(),
            "Le fichier compressé doit être plus petit"
        );
    }

    #[test]
    fn test_decompress_invalid_data() {
        let invalid_data = b"not a zstd stream";

        // Vérifie que notre gestion d'erreur AppError::Io fonctionne
        let result = decompress(invalid_data);

        assert!(
            result.is_err(),
            "La décompression de données invalides doit échouer"
        );
    }
}
