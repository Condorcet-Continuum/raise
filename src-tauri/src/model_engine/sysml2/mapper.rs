// FICHIER : src-tauri/src/model_engine/sysml2/mapper.rs

use crate::utils::prelude::*;

use super::parser::{Rule, Sysml2Parser};
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
use pest::Parser;

#[derive(Default)]
pub struct Sysml2ToArcadiaMapper;

impl Sysml2ToArcadiaMapper {
    pub fn new() -> Self {
        Self
    }

    pub fn transform(&self, sysml_content: &str) -> RaiseResult<ProjectModel> {
        let mut model = ProjectModel::default();

        let parsed_file = Sysml2Parser::parse(Rule::file, sysml_content)
            .map_err(|e| format!("Erreur de syntaxe SysML v2: {}", e))?
            .next()
            .unwrap();

        self.traverse_ast(parsed_file, &mut model, "UnknownLayer");
        Ok(model)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn traverse_ast(
        &self,
        pair: pest::iterators::Pair<Rule>,
        model: &mut ProjectModel,
        current_layer: &str,
    ) {
        match pair.as_rule() {
            Rule::package_decl => {
                let mut inner_rules = pair.into_inner();
                let pkg_name = inner_rules.next().unwrap().as_str();

                for inner_pair in inner_rules {
                    self.traverse_ast(inner_pair, model, pkg_name);
                }
            }
            // On groupe toutes les déclarations qui ont un identifiant simple
            Rule::requirement_def
            | Rule::constraint_def
            | Rule::state_def
            | Rule::item_def
            | Rule::part_def
            | Rule::actor_def
            | Rule::action_def => {
                let rule_type = pair.as_rule();
                // On extrait l'identifiant (ex: "Client", "Req001")
                let ident = pair
                    .into_inner()
                    .find(|p| p.as_rule() == Rule::ident)
                    .map(|p| p.as_str())
                    .unwrap_or("Unknown");

                // On déduit la sémantique Arcadia exacte (le "kind")
                let kind = match rule_type {
                    Rule::requirement_def => "Requirement",
                    Rule::constraint_def => "Constraint",
                    Rule::state_def => "State",
                    Rule::item_def => {
                        if current_layer == "DataDefinition" {
                            "DataClass"
                        } else {
                            "ExchangeItem"
                        }
                    }
                    Rule::part_def => match current_layer {
                        "SystemAnalysis" => "SystemComponent",
                        "LogicalArchitecture" => "LogicalComponent",
                        "PhysicalArchitecture" => "PhysicalComponent",
                        "EPBSArchitecture" => "ConfigurationItem",
                        _ => "Component",
                    },
                    Rule::actor_def => match current_layer {
                        "SystemAnalysis" => "SystemActor",
                        "LogicalArchitecture" => "LogicalActor",
                        "PhysicalArchitecture" => "PhysicalActor",
                        _ => "OperationalActor",
                    },
                    Rule::action_def => {
                        if current_layer == "OperationalAnalysis" {
                            "OperationalActivity"
                        } else {
                            "Function"
                        }
                    }
                    _ => "Unknown",
                };

                // On UTILISE nos imports pour créer l'élément !
                let prefix = current_layer
                    .chars()
                    .take(2)
                    .collect::<String>()
                    .to_lowercase();
                let element = ArcadiaElement {
                    id: format!(
                        "{}-{}-{}",
                        prefix,
                        kind.to_lowercase(),
                        ident.to_lowercase()
                    ),
                    name: NameType::String(ident.to_string()),
                    kind: kind.to_string(),
                    ..Default::default()
                };

                println!(
                    "Création Element [{}]: {:?} ({})",
                    current_layer, element.name, element.kind
                );

                // TODO: Il ne restera plus qu'à faire un push dans le bon vecteur de "model" ici
            }
            Rule::use_case_def => {
                // Géré séparément car l'identifiant peut être une string (ex: 'Accélérer la production')
                println!("Transverse: Ajout Cas d'Usage/Capacité");
            }
            _ => {
                for inner_pair in pair.into_inner() {
                    self.traverse_ast(inner_pair, model, current_layer);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_all_arcadia_layers() {
        let mapper = Sysml2ToArcadiaMapper::new();

        // CORRECTION : "part def SubsystemA { }" avec des accolades au lieu du point-virgule
        let sysml_input = "
        package DataDefinition { item def StructCode { } }
        package TransverseElements { requirement def Req01 { } }
        package OperationalAnalysis { actor def Client; }
        package EPBSArchitecture { part def SubsystemA { } } 
        ";

        // En utilisant unwrap(), si la syntaxe est fausse, Pest affichera l'erreur exacte dans le terminal !
        let _result = mapper.transform(sysml_input).unwrap();
    }
}
