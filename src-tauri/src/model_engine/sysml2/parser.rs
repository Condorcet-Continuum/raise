// FICHIER : src-tauri/src/model_engine/sysml2/parser.rs

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "model_engine/sysml2/grammar.pest"]
pub struct Sysml2Parser;

pub fn parse_sysml_text(
    input: &str,
) -> Result<pest::iterators::Pairs<'_, Rule>, pest::error::Error<Rule>> {
    Sysml2Parser::parse(Rule::file, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_layer() {
        let input = "
        package DataDefinition {
            item def RawCode { attribute language: String; }
        }";
        assert!(
            parse_sysml_text(input).is_ok(),
            "Doit parser les éléments du dictionnaire de données"
        );
    }

    #[test]
    fn test_parse_transverse_layer() {
        let input = "
        package TransverseElements {
            requirement def Req001 { doc /* Le système doit être sécurisé */ }
            constraint def MaxLatency { 'latency < 200ms' }
            state def Idle { }
            use case def 'Garantir la sécurité' { include action AuditCode; }
        }";
        assert!(
            parse_sysml_text(input).is_ok(),
            "Doit parser les exigences, contraintes, états et cas d'usage"
        );
    }

    #[test]
    fn test_parse_epbs_layer() {
        let input = "
        package EPBSArchitecture {
            part def ServerConfigurationItem { }
        }";
        assert!(
            parse_sysml_text(input).is_ok(),
            "Doit parser l'EPBS en utilisant des part def"
        );
    }

    #[test]
    fn test_parse_architecture_layers() {
        let input = "
        package SystemAnalysis { part def System { port api; } }
        package LogicalArchitecture { part def LogicalNode { } }
        package PhysicalArchitecture { part def PhysicalHardware { } }
        ";
        assert!(
            parse_sysml_text(input).is_ok(),
            "Doit parser les architectures SA, LA et PA"
        );
    }
}
