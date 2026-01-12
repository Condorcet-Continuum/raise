// FICHIER : src-tauri/src/model_engine/capella/xmi_parser.rs

use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::path::Path;

pub struct CapellaXmiParser;

impl CapellaXmiParser {
    /// Parse un fichier .capella et remplit le ProjectModel donné
    pub fn parse_file(path: &Path, model: &mut ProjectModel) -> Result<()> {
        let mut reader =
            Reader::from_file(path).context("Impossible d'ouvrir le fichier .capella")?;
        // CORRECTION API Quick-XML
        reader.config_mut().trim_text(true);

        Self::parse_xml(&mut reader, model)
    }

    /// Logique de parsing XML générique (utilisable avec une chaîne pour les tests)
    fn parse_xml<B: std::io::BufRead>(
        reader: &mut Reader<B>,
        model: &mut ProjectModel,
    ) -> Result<()> {
        let mut buf = Vec::new();

        // Boucle de lecture des événements XML
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    // 1. Extraction des attributs communs (id, name, type)
                    let mut id = String::new();
                    let mut name = String::new();
                    let mut xsi_type = String::new();
                    let mut properties = HashMap::new();

                    for a in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(a.key.into_inner()).to_string();
                        let value = String::from_utf8_lossy(&a.value).to_string();

                        match key.as_str() {
                            "id" => id = value,
                            "name" => name = value,
                            "xsi:type" => xsi_type = value,
                            _ => {
                                properties.insert(key, serde_json::Value::String(value));
                            }
                        }
                    }

                    // 2. Si on a un ID et un Type, on crée l'élément
                    if !id.is_empty() && !xsi_type.is_empty() {
                        let element = ArcadiaElement {
                            id: id.clone(),
                            name: NameType::String(if name.is_empty() {
                                "Unnamed".to_string()
                            } else {
                                name
                            }),
                            kind: xsi_type.clone(),
                            // CORRECTION : Ajout du champ manquant
                            description: None,
                            properties,
                        };

                        // 3. Dispatch dans la bonne couche du modèle selon le type XMI
                        Self::dispatch(model, element, &xsi_type);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Erreur XML à la position {}: {:?}",
                        reader.buffer_position(),
                        e
                    ))
                }
                _ => (),
            }
            buf.clear();
        }

        Ok(())
    }

    /// Trie les éléments dans les bons vecteurs du ProjectModel
    fn dispatch(model: &mut ProjectModel, element: ArcadiaElement, xsi_type: &str) {
        // --- OPERATIONAL ANALYSIS (OA) ---
        if xsi_type.contains("oa:OperationalActor") {
            model.oa.actors.push(element);
        } else if xsi_type.contains("oa:OperationalActivity") {
            model.oa.activities.push(element);
        } else if xsi_type.contains("oa:OperationalCapability") {
            model.oa.capabilities.push(element);

        // --- SYSTEM ANALYSIS (SA) ---
        } else if xsi_type.contains("ctx:SystemFunction") {
            model.sa.functions.push(element);
        } else if xsi_type.contains("ctx:SystemComponent") || xsi_type.contains("ctx:System") {
            model.sa.components.push(element);
        } else if xsi_type.contains("ctx:Actor") {
            model.sa.actors.push(element);

        // --- LOGICAL ARCHITECTURE (LA) ---
        } else if xsi_type.contains("la:LogicalFunction") {
            model.la.functions.push(element);
        } else if xsi_type.contains("la:LogicalComponent") {
            model.la.components.push(element);

        // --- PHYSICAL ARCHITECTURE (PA) ---
        } else if xsi_type.contains("pa:PhysicalFunction") {
            model.pa.functions.push(element);
        } else if xsi_type.contains("pa:PhysicalComponent") {
            model.pa.components.push(element);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::reader::Reader;

    #[test]
    fn test_parse_capella_fragment() {
        let xml = r#"
            <root>
                <ownedArchitectures xsi:type="org.polarsys.capella.core.data.la:LogicalArchitecture">
                    <ownedLogicalComponents xsi:type="org.polarsys.capella.core.data.la:LogicalComponent" id="LC_1" name="EngineController" />
                    <ownedLogicalFunctions xsi:type="org.polarsys.capella.core.data.la:LogicalFunction" id="LF_1" name="ComputeThrust" />
                </ownedArchitectures>
                <ownedArchitectures xsi:type="org.polarsys.capella.core.data.ctx:SystemAnalysis">
                     <ownedSystemComponentPkg>
                        <ownedSystemComponents xsi:type="org.polarsys.capella.core.data.ctx:SystemComponent" id="SC_1" name="DroneSystem" />
                     </ownedSystemComponentPkg>
                </ownedArchitectures>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        // CORRECTION API Quick-XML dans le test aussi
        reader.config_mut().trim_text(true);

        let mut model = ProjectModel::default();
        CapellaXmiParser::parse_xml(&mut reader, &mut model).expect("Parsing failed");

        // Vérification LA
        assert_eq!(model.la.components.len(), 1);
        assert_eq!(model.la.components[0].name.as_str(), "EngineController");
        assert_eq!(model.la.functions.len(), 1);

        // Vérification SA
        assert_eq!(model.sa.components.len(), 1);
        assert_eq!(model.sa.components[0].id, "SC_1");
    }
}
