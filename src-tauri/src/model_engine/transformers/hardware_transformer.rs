use super::ModelTransformer;
use anyhow::Result;
use serde_json::{json, Value};

pub struct HardwareTransformer;

impl ModelTransformer for HardwareTransformer {
    fn transform(&self, element: &Value) -> Result<Value> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownModule");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let mut ports = Vec::new();

        // 1. Ports Standards (Clock & Reset)
        // Indispensables pour tout module synchrone en VHDL/Verilog
        ports.push(json!({ "name": "clk", "dir": "in", "type": "std_logic", "description": "System Clock" }));
        ports.push(json!({ "name": "rst_n", "dir": "in", "type": "std_logic", "description": "Active Low Reset" }));

        // 2. Transformation des Échanges Entrants -> Input Ports
        if let Some(incoming) = element
            .get("incomingFunctionalExchanges")
            .and_then(|v| v.as_array())
        {
            for exchange in incoming {
                let port_name = exchange
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unk_in");
                let ex_id = exchange.get("id").and_then(|v| v.as_str()).unwrap_or("");

                ports.push(json!({
                    "name": port_name.to_lowercase(),
                    "dir": "in",
                    // Type par défaut, à affiner si le modèle de données (DataLayer) est lié
                    "type": "std_logic_vector(31 downto 0)",
                    "id": ex_id
                }));
            }
        }

        // 3. Transformation des Échanges Sortants -> Output Ports
        if let Some(outgoing) = element
            .get("outgoingFunctionalExchanges")
            .and_then(|v| v.as_array())
        {
            for exchange in outgoing {
                let port_name = exchange
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unk_out");
                let ex_id = exchange.get("id").and_then(|v| v.as_str()).unwrap_or("");

                ports.push(json!({
                    "name": port_name.to_lowercase(),
                    "dir": "out",
                    "type": "std_logic_vector(31 downto 0)",
                    "id": ex_id
                }));
            }
        }

        // 4. Transformation des Fonctions -> Processus internes / Blocks
        // En HW, une fonction devient souvent un commentaire de section ou un process
        let mut processes = Vec::new();
        if let Some(funcs) = element
            .get("ownedFunctionalAllocation")
            .and_then(|v| v.as_array())
        {
            for func in funcs {
                if let Some(fname) = func.get("name").and_then(|v| v.as_str()) {
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

    #[test]
    fn test_hardware_transformation_ports_generation() {
        let transformer = HardwareTransformer;

        // Mock d'un composant FPGA hydraté avec flux
        let component = json!({
            "id": "UUID_FPGA_1",
            "name": "VideoProcessor",
            "incomingFunctionalExchanges": [
                { "id": "EX_1", "name": "PixelDataIn" }
            ],
            "outgoingFunctionalExchanges": [
                { "id": "EX_2", "name": "HDMIOut" }
            ]
        });

        let result = transformer
            .transform(&component)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "hardware");

        let ports = result["module"]["ports"].as_array().expect("Ports missing");

        // Vérifie la présence des ports standards (clk, rst_n) + fonctionnels (PixelDataIn, HDMIOut)
        // 2 standards + 2 fonctionnels = 4 ports
        assert_eq!(ports.len(), 4);

        // Vérification des signaux standards
        assert!(ports.iter().any(|p| p["name"] == "clk" && p["dir"] == "in"));
        assert!(ports
            .iter()
            .any(|p| p["name"] == "rst_n" && p["dir"] == "in"));

        // Vérification direction Flux
        let pixel_in = ports
            .iter()
            .find(|p| p["name"] == "pixeldatain")
            .expect("PixelIn missing");
        assert_eq!(pixel_in["dir"], "in");

        let hdmi_out = ports
            .iter()
            .find(|p| p["name"] == "hdmiout")
            .expect("HDMIOut missing");
        assert_eq!(hdmi_out["dir"], "out");
    }
}
