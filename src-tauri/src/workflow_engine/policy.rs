use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashSet;

// On importe les types de base nécessaires
// Si ComplianceLevel n'existe pas dans traceability, on le définit ici pour l'instant
// pour éviter les dépendances circulaires ou cassées.

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub enum ComplianceLevel {
    #[default]
    None,      // Pas de restriction (Dev)
    Basic,     // Logs simples
    High,      // Validation règles métier requise
    Critical,  // Double validation (Humaine ou IA Tierce)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPermissions {
    /// Outils autorisés explicitement (Allowlist)
    pub allowed_tools: HashSet<String>,
    
    /// Catégories d'actions sensibles
    pub can_read_files: bool,
    pub can_write_files: bool,
    pub can_execute_commands: bool,
    pub can_access_network: bool,
}

impl Default for McpPermissions {
    fn default() -> Self {
        Self {
            allowed_tools: HashSet::new(),
            can_read_files: true, // Souvent nécessaire par défaut
            can_write_files: false,
            can_execute_commands: false,
            can_access_network: false,
        }
    }
}

/// Le Mandat est passé au WorkflowEngine lors de l'initialisation d'un job.
/// Il dicte ce que ce job spécifique a le droit de faire.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mandate {
    pub id: Uuid,
    pub role: String,
    pub compliance_level: ComplianceLevel,
    pub permissions: McpPermissions,
}

impl Mandate {
    pub fn new(role: &str, level: ComplianceLevel) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: role.to_string(),
            compliance_level: level,
            permissions: McpPermissions::default(),
        }
    }

    /// Vérifie si une action spécifique est autorisée par ce mandat
    pub fn authorizes_tool(&self, tool_name: &str) -> bool {
        // Si on est en mode "None" (Dev), tout est permis (ou logique inverse selon votre sécu)
        if self.compliance_level == ComplianceLevel::None {
            return true;
        }
        
        // Sinon, on vérifie la whitelist
        self.permissions.allowed_tools.contains(tool_name)
    }
    
    // Builders pour faciliter la construction dans les tests existants
    pub fn with_write_access(mut self) -> Self {
        self.permissions.can_write_files = true;
        self
    }
    
    pub fn allow_tool(mut self, tool: &str) -> Self {
        self.permissions.allowed_tools.insert(tool.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandate_enforcement() {
        let mut mandate = Mandate::new("TestAgent", ComplianceLevel::High);
        
        // Par défaut, rien n'est permis en High
        assert_eq!(mandate.authorizes_tool("fs_delete"), false);
        
        // On ajoute une permission
        mandate = mandate.allow_tool("fs_delete");
        assert_eq!(mandate.authorizes_tool("fs_delete"), true);
    }
}