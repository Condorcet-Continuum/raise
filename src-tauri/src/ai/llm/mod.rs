use self::candle_engine::CandleLlmEngine;
use crate::utils::Mutex;

pub mod candle_engine;
pub mod client;
pub mod prompts;
pub mod response_parser;

#[cfg(test)]
mod tests;

// Structure qui porte l'état du moteur natif
// On utilise Option car au démarrage de l'app, le moteur n'est pas encore chargé.
pub struct NativeLlmState(pub Mutex<Option<CandleLlmEngine>>);
