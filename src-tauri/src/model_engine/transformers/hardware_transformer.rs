// FICHIER : src-tauri/src/model_engine/transformers/hardware_transformer.rs
use crate::utils::prelude::*;

use super::ModelTransformer;
use crate::model_engine::arcadia; // <-- Accès au vocabulaire

pub struct HardwareTransformer;

impl ModelTransformer for HardwareTransformer {
    fn transform(&self, element: &Value) -> RaiseResult<Value> {
        // Utilisation des constantes pour les champs standards
        let name = element
            .get(arcadia::PROP_NAME)
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownModule");

        let id = element
            .get(arcadia::PROP_ID)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut ports = Vec::new();

        // 1. Ports Standards (Clock & Reset)
        ports.push(json!({ "name": "clk", "dir": "in", "type": "std_logic", "description": "System Clock" }));
        ports.push(json!({ "name": "rst_n", "dir": "in", "type": "std_logic", "description": "Active Low Reset" }));

        // 2. Transformation des Échanges Entrants -> Input Ports
        if let Some(incoming) = element
            .get(arcadia::PROP_INCOMING_EXCHANGES)
            .and_then(|v| v.as_array())
        {
            for exchange in incoming {
                let port_name = exchange
                    .get(arcadia::PROP_NAME)
                    .and_then(|v| v.as_str())
                    .unwrap_or("unk_in");
                let ex_id = exchange
                    .get(arcadia::PROP_ID)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                ports.push(json!({
                    "name": port_name.to_lowercase(),
                    "dir": "in",
                    "type": "std_logic_vector(31 downto 0)",
                    "id": ex_id
                }));
            }
        }

        // 3. Transformation des Échanges Sortants -> Output Ports
        if let Some(outgoing) = element
            .get(arcadia::PROP_OUTGOING_EXCHANGES)
            .and_then(|v| v.as_array())
        {
            for exchange in outgoing {
                let port_name = exchange
                    .get(arcadia::PROP_NAME)
                    .and_then(|v| v.as_str())
                    .unwrap_or("unk_out");
                let ex_id = exchange
                    .get(arcadia::PROP_ID)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                ports.push(json!({
                    "name": port_name.to_lowercase(),
                    "dir": "out",
                    "type": "std_logic_vector(31 downto 0)",
                    "id": ex_id
                }));
            }
        }

        // 4. Transformation des Fonctions -> Processus internes / Blocks
        // CORRECTION : Initialisation du vecteur manquante
        let mut processes = Vec::new();

        let alloc_key = "ownedFunctionalAllocation";

        if let Some(funcs) = element.get(alloc_key).and_then(|v| v.as_array()) {
            for func in funcs {
                if let Some(fname) = func.get(arcadia::PROP_NAME).and_then(|v| v.as_str()) {
                    processes.push(json!({
                        "name": fname,
                        "description": "Logique séquentielle pour cette fonction"
                    }));
                }
            }
        }

        // Structure optimisée pour Tera (VHDL/Verilog)
        Ok(json!({
            "domain": "hardware",
            "meta": {
                "uuid": id,
                "source_element": name,
                "generated_at": chrono::Utc::now().to_rfc3339()
            },
            "module": {
                "name": name, // Sera utilisé comme Entity Name
                "ports": ports,
                "processes": processes
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia;

    #[test]
    fn test_hardware_transformation_ports_generation() {
        let transformer = HardwareTransformer;

        let component = json!({
            arcadia::PROP_ID: "UUID_FPGA_1",
            arcadia::PROP_NAME: "VideoProcessor",
            arcadia::PROP_INCOMING_EXCHANGES: [
                { arcadia::PROP_ID: "EX_1", arcadia::PROP_NAME: "PixelDataIn" }
            ],
            arcadia::PROP_OUTGOING_EXCHANGES: [
                { arcadia::PROP_ID: "EX_2", arcadia::PROP_NAME: "HDMIOut" }
            ]
        });

        let result = transformer
            .transform(&component)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "hardware");

        let ports = result["module"]["ports"].as_array().expect("Ports missing");

        assert_eq!(ports.len(), 4);
    }
}
