// FICHIER : src-tauri/src/model_engine/capella/xmi_parser.rs

use crate::model_engine::arcadia::ArcadiaOntology; // 🎯 Utilisation de l'ontologie dynamique
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
use crate::utils::prelude::*;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

pub struct CapellaXmiParser;

impl CapellaXmiParser {
    pub fn parse_file(path: &Path, model: &mut ProjectModel) -> RaiseResult<()> {
        let mut reader = match Reader::from_file(path) {
            Ok(r) => r,
            Err(e) => raise_error!(
                "ERR_XMI_READ_FAIL",
                error = e,
                context = json_value!({
                    "path": path.display().to_string(),
                    "format": "XMI/XML",
                    "action": "initialize_reader"
                })
            ),
        };
        reader.config_mut().trim_text(true);
        Self::parse_xml(&mut reader, model)
    }

    fn parse_xml<B: BufferedRead>(
        reader: &mut Reader<B>,
        model: &mut ProjectModel,
    ) -> RaiseResult<()> {
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let mut id = String::new();
                    let mut name = String::new();
                    let mut xsi_type = String::new();
                    let mut properties = UnorderedMap::new();

                    for a in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(a.key.into_inner()).to_string();
                        let value = String::from_utf8_lossy(&a.value).to_string();

                        match key.as_str() {
                            "id" => id = value,
                            "name" => name = value,
                            "xsi:type" => xsi_type = value,
                            _ => {
                                properties.insert(key, JsonValue::String(value));
                            }
                        }
                    }

                    if !id.is_empty() && !xsi_type.is_empty() {
                        let element = ArcadiaElement {
                            id: id.clone(),
                            name: NameType::String(if name.is_empty() {
                                "Unnamed".to_string()
                            } else {
                                name
                            }),
                            kind: xsi_type.clone(),
                            description: None,
                            properties,
                        };

                        Self::dispatch(model, element, &xsi_type);
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    let pos = reader.buffer_position();
                    raise_error!(
                        "ERR_XML_PARSE_FAILURE",
                        error = format!("Erreur XML à {} : {}", pos, e)
                    );
                }
                _ => (),
            }
            buf.clear();
        }
        Ok(())
    }

    /// Trie les éléments en résolvant dynamiquement les URIs Raise
    fn dispatch(model: &mut ProjectModel, mut element: ArcadiaElement, xsi_type: &str) {
        // Fonction helper pour résoudre l'URI via l'ontologie dynamique
        let resolve = |prefix: &str, name: &str| -> String {
            ArcadiaOntology::get_uri(prefix, name).unwrap_or_else(|| xsi_type.to_string())
        };

        // --- OPERATIONAL ANALYSIS (OA) ---
        if xsi_type.contains("oa:OperationalActor") {
            element.kind = resolve("oa", "OperationalActor");
            model.oa.actors.push(element);
        } else if xsi_type.contains("oa:OperationalActivity") {
            element.kind = resolve("oa", "OperationalActivity");
            model.oa.activities.push(element);
        } else if xsi_type.contains("oa:OperationalCapability") {
            element.kind = resolve("oa", "OperationalCapability");
            model.oa.capabilities.push(element);
        } else if xsi_type.contains("oa:Entity") || xsi_type.contains("oa:OperationalEntity") {
            element.kind = resolve("oa", "OperationalEntity");
            model.oa.entities.push(element);

        // --- SYSTEM ANALYSIS (SA) ---
        } else if xsi_type.contains("ctx:SystemFunction") {
            element.kind = resolve("sa", "SystemFunction");
            model.sa.functions.push(element);
        } else if xsi_type.contains("ctx:SystemComponent") || xsi_type.contains("ctx:System") {
            element.kind = resolve("sa", "SystemComponent");
            model.sa.components.push(element);
        } else if xsi_type.contains("ctx:Actor") {
            element.kind = resolve("sa", "SystemActor");
            model.sa.actors.push(element);

        // --- LOGICAL ARCHITECTURE (LA) ---
        } else if xsi_type.contains("la:LogicalFunction") {
            element.kind = resolve("la", "LogicalFunction");
            model.la.functions.push(element);
        } else if xsi_type.contains("la:LogicalComponent") {
            element.kind = resolve("la", "LogicalComponent");
            model.la.components.push(element);

        // --- PHYSICAL ARCHITECTURE (PA) ---
        } else if xsi_type.contains("pa:PhysicalFunction") {
            element.kind = resolve("pa", "PhysicalFunction");
            model.pa.functions.push(element);
        } else if xsi_type.contains("pa:PhysicalComponent") {
            element.kind = resolve("pa", "PhysicalComponent");
            model.pa.components.push(element);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::ArcadiaOntology;

    #[test]
    fn test_parse_capella_fragment_and_normalize() {
        let xml = r#"
            <root>
                <ownedArchitectures xsi:type="org.polarsys.capella.core.data.la:LogicalArchitecture">
                    <ownedLogicalComponents xsi:type="org.polarsys.capella.core.data.la:LogicalComponent" id="LC_1" name="EngineController" />
                </ownedArchitectures>
            </root>
        "#;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut model = ProjectModel::default();
        CapellaXmiParser::parse_xml(&mut reader, &mut model).expect("Parsing failed");

        let comp = &model.la.components[0];
        assert_eq!(comp.name.as_str(), "EngineController");

        // 🎯 Vérification : Le type doit correspondre à ce que le registre renvoie dynamiquement
        let expected_uri = ArcadiaOntology::get_uri("la", "LogicalComponent").unwrap();
        assert_eq!(comp.kind, expected_uri);
    }
}
