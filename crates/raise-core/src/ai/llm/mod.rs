use crate::utils::prelude::*;

use self::native_engine::NativeTensorEngine;
pub mod client;
pub mod native_engine;
pub mod providers;
pub mod response_parser;

#[cfg(test)]
mod tests;

// Structure qui porte l'état du moteur natif
// On utilise Option car au démarrage de l'app, le moteur n'est pas encore chargé.
pub struct NativeLlmState(pub SyncMutex<Option<NativeTensorEngine>>);
