// FICHIER : src-tauri/src/code_generator/templates/template_engine.rs

use crate::utils::data::{HashMap, Value};
use crate::utils::io::{Path, ProjectScope};
use crate::utils::prelude::*; // AppError, Result, info!
use tera::{try_get_value, Tera};

pub struct TemplateEngine {
    tera: Tera,
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut tera = Tera::default();

        // 1. Filtres
        tera.register_filter("pascal_case", filters::pascal_case_filter);
        tera.register_filter("snake_case", filters::snake_case_filter);
        tera.register_filter("camel_case", filters::camel_case_filter);
        tera.register_filter("screaming_snake_case", filters::screaming_snake_case_filter);

        // 2. Templates
        register_default_templates(&mut tera);

        Self { tera }
    }

    pub fn render(&self, template_name: &str, context: &Value) -> Result<String> {
        let tera_ctx = tera::Context::from_value(context.clone())
            .map_err(|e| AppError::System(anyhow::anyhow!("Tera Context Error: {}", e)))?;

        self.tera.render(template_name, &tera_ctx).map_err(|e| {
            AppError::System(anyhow::anyhow!(
                "Tera Render Error [{}]: {}",
                template_name,
                e
            ))
        })
    }

    /// 🚀 GÉNÉRATION PHYSIQUE SÉCURISÉE
    pub async fn generate(
        &self,
        scope: &ProjectScope,
        template_name: &str,
        context: &Value,
        relative_path: impl AsRef<Path>,
    ) -> Result<()> {
        let content = self.render(template_name, context)?;
        scope
            .write(relative_path.as_ref(), content.as_bytes())
            .await?;

        info!(
            target: "codegen",
            "📝 Généré : {:?} (via {})",
            relative_path.as_ref(),
            template_name
        );
        Ok(())
    }

    pub fn add_raw_template(&mut self, name: &str, content: &str) -> Result<()> {
        self.tera
            .add_raw_template(name, content)
            .map_err(|e| AppError::System(anyhow::anyhow!("Invalid Template '{}': {}", name, e)))?;
        Ok(())
    }
}

mod filters {
    use super::*;
    use heck::{ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase};
    use tera::{to_value, Value};

    pub fn pascal_case_filter(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = try_get_value!("pascal_case", "value", String, value);
        Ok(to_value(s.to_pascal_case()).unwrap())
    }

    pub fn snake_case_filter(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = try_get_value!("snake_case", "value", String, value);
        Ok(to_value(s.to_snake_case()).unwrap())
    }

    pub fn camel_case_filter(value: &Value, _: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = try_get_value!("camel_case", "value", String, value);
        Ok(to_value(s.to_lower_camel_case()).unwrap())
    }

    pub fn screaming_snake_case_filter(
        value: &Value,
        _: &HashMap<String, Value>,
    ) -> tera::Result<Value> {
        let s = try_get_value!("screaming_snake_case", "value", String, value);
        Ok(to_value(s.to_shouty_snake_case()).unwrap())
    }
}

fn register_default_templates(tera: &mut Tera) {
    // --- RUST (Corrigé pour l'injection !) ---
    tera.add_raw_template(
        "rust/actor",
        r#"
// GÉNÉRÉ PAR RAISE
use serde::{Deserialize, Serialize};

/// {{ description | default(value="Aucune description") }}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{ name | pascal_case }} {
    pub id: String,
}

impl {{ name | pascal_case }} {
    pub fn new() -> Self {
        Self { id: "{{ id }}".to_string() }
    }

    // AI_INJECTION_POINT: Logic
    // END_AI_INJECTION_POINT
}
"#,
    )
    .unwrap();

    // --- CPP HEADER ---
    tera.add_raw_template(
        "cpp/header",
        r#"
#pragma once
#include <string>
class {{ name | pascal_case }} {
public:
    {{ name | pascal_case }}();
private:
    std::string id = "{{ id }}";
};
"#,
    )
    .unwrap();

    // --- CPP SOURCE ---
    tera.add_raw_template(
        "cpp/source",
        r#"
#include "{{ name | pascal_case }}.hpp"
{{ name | pascal_case }}::{{ name | pascal_case }}() {}
"#,
    )
    .unwrap();

    // --- TYPESCRIPT ---
    tera.add_raw_template(
        "ts/class",
        r#"
export class {{ name | pascal_case }} {
    public id: string = "{{ id }}";
}
"#,
    )
    .unwrap();

    // --- VERILOG ---
    tera.add_raw_template(
        "verilog/module",
        r#"
module {{ name | snake_case }} (
    input wire clk,
    input wire rst_n
);
    // {{ description | default(value="") }}
endmodule
"#,
    )
    .unwrap();

    // --- VHDL ---
    tera.add_raw_template(
        "vhdl/entity",
        r#"
entity {{ name | snake_case }} is
    Port ( clk : in STD_LOGIC; rst_n : in STD_LOGIC );
end {{ name | snake_case }};

architecture Behavioral of {{ name | snake_case }} is
begin
    -- {{ description | default(value="") }}
end Behavioral;
"#,
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::ContextBuilder;
    use crate::utils::io::tempdir;

    #[tokio::test]
    async fn test_secure_generation() {
        let engine = TemplateEngine::new();
        let dir = tempdir().unwrap();
        let scope = ProjectScope::new(dir.path()).unwrap();

        let ctx = ContextBuilder::new()
            .with_part("name", &"SecureActor")
            .with_part("id", &"SA_007")
            .build();

        let res = engine
            .generate(&scope, "rust/actor", &ctx, "src/actors/secure_actor.rs")
            .await;
        assert!(res.is_ok());

        let file_path = dir.path().join("src/actors/secure_actor.rs");
        assert!(file_path.exists());
    }
}
