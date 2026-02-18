// FICHIER : src-tauri/src/model_engine/capella/xmi_parser.rs

use crate::utils::{prelude::*, HashMap};

use crate::model_engine::arcadia; // <-- Import du vocabulaire cible
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

pub struct CapellaXmiParser;

impl CapellaXmiParser {
    /// Parse un fichier .capella et remplit le ProjectModel donné
    pub fn parse_file(path: &Path, model: &mut ProjectModel) -> Result<()> {
        let mut reader = Reader::from_file(path).map_err(|e| {
            crate::utils::AppError::from(format!("Impossible de lire le fichier XMI : {}", e))
        })?;

        reader.config_mut().trim_text(true);

        Self::parse_xml(&mut reader, model)
    }

    /// Logique de parsing XML générique
    fn parse_xml<B: std::io::BufRead>(
        reader: &mut Reader<B>,
        model: &mut ProjectModel,
    ) -> Result<()> {
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
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

                    if !id.is_empty() && !xsi_type.is_empty() {
                        // On crée l'élément avec le type brut pour l'instant
                        let element = ArcadiaElement {
                            id: id.clone(),
                            name: NameType::String(if name.is_empty() {
                                "Unnamed".to_string()
                            } else {
                                name
                            }),
                            kind: xsi_type.clone(), // Sera normalisé dans dispatch()
                            description: None,
                            properties,
                        };

                        // Dispatch et Normalisation
                        Self::dispatch(model, element, &xsi_type);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(AppError::Validation(format!(
                        "Erreur XML à la position {}: {:?}",
                        reader.buffer_position(),
                        e
                    )))
                }
                _ => (),
            }
            buf.clear();
        }

        Ok(())
    }

    /// Trie les éléments et normalise leur 'kind' vers les URIs Raise
    fn dispatch(model: &mut ProjectModel, mut element: ArcadiaElement, xsi_type: &str) {
        // --- OPERATIONAL ANALYSIS (OA) ---
        if xsi_type.contains("oa:OperationalActor") {
            element.kind = arcadia::KIND_OA_ACTOR.to_string();
            model.oa.actors.push(element);
        } else if xsi_type.contains("oa:OperationalActivity") {
            element.kind = arcadia::KIND_OA_ACTIVITY.to_string();
            model.oa.activities.push(element);
        } else if xsi_type.contains("oa:OperationalCapability") {
            element.kind = arcadia::KIND_OA_CAPABILITY.to_string();
            model.oa.capabilities.push(element);
        } else if xsi_type.contains("oa:Entity") || xsi_type.contains("oa:OperationalEntity") {
            element.kind = arcadia::KIND_OA_ENTITY.to_string();
            model.oa.entities.push(element);

        // --- SYSTEM ANALYSIS (SA) ---
        } else if xsi_type.contains("ctx:SystemFunction") {
            element.kind = arcadia::KIND_SA_FUNCTION.to_string();
            model.sa.functions.push(element);
        } else if xsi_type.contains("ctx:SystemComponent") || xsi_type.contains("ctx:System") {
            element.kind = arcadia::KIND_SA_COMPONENT.to_string();
            model.sa.components.push(element);
        } else if xsi_type.contains("ctx:Actor") {
            element.kind = arcadia::KIND_SA_ACTOR.to_string();
            model.sa.actors.push(element);

        // --- LOGICAL ARCHITECTURE (LA) ---
        } else if xsi_type.contains("la:LogicalFunction") {
            element.kind = arcadia::KIND_LA_FUNCTION.to_string();
            model.la.functions.push(element);
        } else if xsi_type.contains("la:LogicalComponent") {
            element.kind = arcadia::KIND_LA_COMPONENT.to_string();
            model.la.components.push(element);

        // --- PHYSICAL ARCHITECTURE (PA) ---
        } else if xsi_type.contains("pa:PhysicalFunction") {
            element.kind = arcadia::KIND_PA_FUNCTION.to_string();
            model.pa.functions.push(element);
        } else if xsi_type.contains("pa:PhysicalComponent") {
            element.kind = arcadia::KIND_PA_COMPONENT.to_string();
            model.pa.components.push(element);
        }
        // Sinon : On ignore ou on stocke tel quel si besoin (EPBS, Data...)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia; // Pour vérifier la normalisation
    use quick_xml::reader::Reader;

    #[test]
    fn test_parse_capella_fragment_and_normalize() {
        let xml = r#"
            <root>
                <ownedArchitectures xsi:type="org.polarsys.capella.core.data.la:LogicalArchitecture">
                    <ownedLogicalComponents xsi:type="org.polarsys.capella.core.data.la:LogicalComponent" id="LC_1" name="EngineController" />
                    <ownedLogicalFunctions xsi:type="org.polarsys.capella.core.data.la:LogicalFunction" id="LF_1" name="ComputeThrust" />
                </ownedArchitectures>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut model = ProjectModel::default();
        CapellaXmiParser::parse_xml(&mut reader, &mut model).expect("Parsing failed");

        // Vérification LA
        assert_eq!(model.la.components.len(), 1);
        let comp = &model.la.components[0];

        assert_eq!(comp.name.as_str(), "EngineController");
        // Vérification CRITIQUE : Le kind doit être l'URI Raise, pas le type Capella
        assert_eq!(comp.kind, arcadia::KIND_LA_COMPONENT);

        let func = &model.la.functions[0];
        assert_eq!(func.kind, arcadia::KIND_LA_FUNCTION);
    }
}
