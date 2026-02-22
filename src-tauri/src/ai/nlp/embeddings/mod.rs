use crate::utils::prelude::*;

pub mod candle;
pub mod fast;

pub enum EngineType {
    FastEmbed,
    Candle,
}

pub struct EmbeddingEngine {
    inner: EngineImplementation,
}

enum EngineImplementation {
    Fast(Box<fast::FastEmbedEngine>),
    Candle(Box<candle::CandleEngine>),
}

impl EmbeddingEngine {
    pub fn new() -> Result<Self> {
        // CHANGEMENT ICI : On passe Candle par défaut pour profiter du GPU
        // Si Candle échoue (ex: pas internet pour télécharger le modèle), on pourrait fallback sur FastEmbed
        println!("🧠 Init NLP Engine: Tentative Candle (GPU)...");
        match Self::new_with_type(EngineType::Candle) {
            Ok(engine) => Ok(engine),
            Err(e) => {
                eprintln!("⚠️ Echec Candle ({}), bascule sur FastEmbed (CPU)", e);
                Self::new_with_type(EngineType::FastEmbed)
            }
        }
    }

    pub fn new_with_type(engine_type: EngineType) -> Result<Self> {
        let inner = match engine_type {
            EngineType::FastEmbed => {
                println!("🧠 Init NLP Engine: FastEmbed (ONNX)");
                let fast_engine = fast::FastEmbedEngine::new()?;
                EngineImplementation::Fast(Box::new(fast_engine))
            }
            EngineType::Candle => {
                println!("🕯️ Init NLP Engine: Candle (BERT Pure Rust)");
                let candle_engine = candle::CandleEngine::new()?;
                EngineImplementation::Candle(Box::new(candle_engine))
            }
        };
        Ok(Self { inner })
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        match &mut self.inner {
            EngineImplementation::Fast(e) => e
                .embed_batch(texts)
                // ✅ Conversion de anyhow::Error vers AppError
                .map_err(|err| AppError::from(err.to_string())),

            EngineImplementation::Candle(e) => e.embed_batch(texts), // ✅ Déjà un AppError
        }
    }

    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        match &mut self.inner {
            EngineImplementation::Fast(e) => e
                .embed_query(text)
                // ✅ Conversion de anyhow::Error vers AppError
                .map_err(|err| AppError::from(err.to_string())),

            EngineImplementation::Candle(e) => e.embed_query(text), // ✅ Déjà un AppError
        }
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_engine_init() {
        let engine = EmbeddingEngine::new();
        assert!(
            engine.is_ok(),
            "Le moteur par défaut doit s'initialiser sans erreur"
        );
    }

    #[test]
    fn test_engine_switching() {
        // Test FastEmbed
        let mut fast_engine =
            EmbeddingEngine::new_with_type(EngineType::FastEmbed).expect("FastEmbed init failed");
        let vec_fast = fast_engine.embed_query("Test Fast").expect("Embed failed");
        assert_eq!(
            vec_fast.len(),
            384,
            "FastEmbed (BGE-Small) doit sortir 384 dims"
        );

        // Test Candle (Si l'environnement le permet, sinon on skip ou on log)
        if let Ok(mut candle_engine) = EmbeddingEngine::new_with_type(EngineType::Candle) {
            let vec_candle = candle_engine
                .embed_query("Test Candle")
                .expect("Embed failed");
            assert_eq!(
                vec_candle.len(),
                384,
                "Candle (MiniLM) doit sortir 384 dims"
            );
        } else {
            println!("⚠️ Candle Engine skipped in tests (might be network/setup related)");
        }
    }
}
