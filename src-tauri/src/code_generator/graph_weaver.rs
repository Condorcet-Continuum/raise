// FICHIER : src-tauri/src/code_generator/graph_weaver.rs

use crate::code_generator::models::{CodeElement, CodeElementType, Module, Visibility};
use crate::code_generator::module_weaver::ModuleWeaver;
use crate::code_generator::toolchains::ToolchainStrategy;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // Trait (ex: RustToolchain)

pub struct OntologyWeaver;

impl OntologyWeaver {
    /// Extrait un élément du Graphe (JSON-LD), génère l'AST, tisse le code et le valide.
    pub async fn generate_and_validate(
        manager: &CollectionsManager<'_>,
        element_id: &str,
        target_path: PathBuf,
        toolchain: &dyn ToolchainStrategy,
    ) -> RaiseResult<PathBuf> {
        user_info!(
            "INF_ONTOLOGY_WEAVE_START",
            json_value!({"element_id": element_id, "target": target_path.to_string_lossy()})
        );

        // ====================================================================
        // 1. EXTRACTION DE LA SOURCE DE VÉRITÉ (Graphe JSON-LD)
        // ====================================================================
        let doc = match manager.get_document("code_elements", element_id).await {
            Ok(Some(d)) => d,
            Ok(None) => raise_error!(
                "ERR_CODEGEN_ELEMENT_NOT_FOUND",
                context = json_value!({"element_id": element_id, "hint": "Le nœud n'existe pas dans le graphe d'architecture physique."})
            ),
            Err(e) => raise_error!(
                "ERR_CODEGEN_DB_READ",
                error = e,
                context = json_value!({"element_id": element_id})
            ),
        };

        // ====================================================================
        // 2. MAPPING SÉMANTIQUE : JSON-LD -> AST (models::CodeElement)
        // ====================================================================
        let properties = match doc.get("properties").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => raise_error!(
                "ERR_CODEGEN_MISSING_PROPERTIES",
                context = json_value!({"element_id": element_id, "hint": "L'élément JSON-LD doit posséder un objet 'properties'."})
            ),
        };

        // Extraction sécurisée des champs (Zéro unwrap)
        let module_name = properties
            .get("module_name")
            .and_then(|v| v.as_str())
            .unwrap_or("generated_module");
        let signature = match properties.get("signature").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => raise_error!(
                "ERR_CODEGEN_MISSING_SIGNATURE",
                context = json_value!({"element_id": element_id})
            ),
        };
        let body = properties
            .get("body")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let visibility_str = properties
            .get("visibility")
            .and_then(|v| v.as_str())
            .unwrap_or("public");
        let visibility = match visibility_str.to_lowercase().as_str() {
            "private" => Visibility::Private,
            "crate" => Visibility::Crate,
            "protected" => Visibility::Protected,
            _ => Visibility::Public,
        };

        // Construction de l'AST Racine
        let mut module = match Module::new(module_name, target_path.clone()) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_CODEGEN_MODULE_INIT",
                error = e,
                context = json_value!({"module_name": module_name})
            ),
        };

        // Ajout de l'élément principal (Dans un cas réel, on parcourrait les downstream_links du Tracer)
        module.elements.push(CodeElement {
            module_id: Some(element_id.to_string()),
            parent_id: None,
            attributes: vec![],
            docs: properties
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| format!("/// {}", s)),
            elements: vec![],
            handle: element_id.to_string(),
            element_type: CodeElementType::Function, // Simplification, à déduire de doc["type"]
            visibility,
            signature,
            body,
            dependencies: vec![], // À extraire du Tracer (ImpactAnalyzer)
            metadata: UnorderedMap::new(),
        });

        // ====================================================================
        // 3. TISSAGE & PERSISTANCE PHYSIQUE
        // ====================================================================
        let saved_path = match ModuleWeaver::sync_to_disk(
            &module,
            target_path.parent().unwrap_or(Path::new("")),
        )
        .await
        {
            Ok(p) => p,
            Err(e) => raise_error!(
                "ERR_CODEGEN_SYNC",
                error = e,
                context = json_value!({"element_id": element_id})
            ),
        };

        // ====================================================================
        // 4. VALIDATION STRICTE (Toolchain)
        // ====================================================================
        let working_dir = saved_path.parent();

        // Formatage silencieux (On ne crash pas si rustfmt échoue, on loggue juste en warning)
        if let Err(e) = toolchain.format(&saved_path).await {
            user_warn!(
                "WRN_CODEGEN_FORMAT_FAILED",
                json_value!({"error": e.to_string(), "path": saved_path.to_string_lossy()})
            );
        }

        // Compilation formelle (Check) -> Si ça crash, ça génère une XaiFrame exploitable
        match toolchain.check(module_name, working_dir).await {
            Ok(_) => user_success!(
                "SUC_CODEGEN_VALIDATED",
                json_value!({"module": module_name})
            ),
            Err(e) => raise_error!(
                "ERR_CODEGEN_TOOLCHAIN_REJECTED",
                error = e,
                context = json_value!({
                    "module": module_name,
                    "hint": "Le compilateur Rust a rejeté l'AST généré. La trace doit être renvoyée à l'Agent."
                })
            ),
        };

        Ok(saved_path)
    }
}
