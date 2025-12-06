use anyhow::Result;
use async_trait::async_trait;

// 1. On déclare les modules qui sont DANS ce dossier (agents/)
pub mod intent_classifier;
pub mod software_agent;
pub mod system_agent;
// pub mod hardware_agent;

// 2. On importe le type depuis le module local (pas de 'super::')
pub use self::intent_classifier::EngineeringIntent;

#[cfg(test)]
mod tests;

/// Le contrat que tous les agents spécialisés doivent respecter
#[async_trait]
pub trait Agent {
    /// Tente de traiter une intention.
    /// Retourne Ok(Some(message)) si traitée, Ok(None) si ignorée, ou Err.
    async fn process(&self, intent: &EngineeringIntent) -> Result<Option<String>>;
}
