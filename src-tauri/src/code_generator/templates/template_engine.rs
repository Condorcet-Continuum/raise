use anyhow::{Context, Result};
use std::collections::HashMap;
use tera::{to_value, try_get_value, Tera, Value};

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

    pub fn render(&self, template_name: &str, context: &tera::Context) -> Result<String> {
        self.tera
            .render(template_name, context)
            .with_context(|| format!("Échec du rendu du template '{}'", template_name))
    }

    pub fn add_raw_template(&mut self, name: &str, content: &str) -> Result<()> {
        self.tera.add_raw_template(name, content)?;
        Ok(())
    }
}

fn register_default_templates(tera: &mut Tera) {
    // --- RUST ---
    tera.add_raw_template(
        "rust/actor",
        r#"
// GÉNÉRÉ PAR RAISE
use serde::{Deserialize, Serialize};

/// {{ description }}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{ name | pascal_case }} {
    pub id: String,
}

impl {{ name | pascal_case }} {
    pub fn new() -> Self {
        Self { id: "{{ id }}".to_string() }
    }
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
    // {{ description }}
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
    -- {{ description }}
end Behavioral;
"#,
    )
    .unwrap();

    // --- C++ HEADER ---
    tera.add_raw_template(
        "cpp/header",
        r#"
/**
 * GÉNÉRÉ PAR RAISE
 * Module: {{ name }}
 * ID: {{ id }}
 */
#pragma once

#include <string>
#include <iostream>

class {{ name | pascal_case }} {
public:
    {{ name | pascal_case }}();
    ~{{ name | pascal_case }}();

    void init();
    void step();

private:
    std::string id = "{{ id }}";
};
"#,
    )
    .unwrap();

    // --- C++ SOURCE ---
    tera.add_raw_template(
        "cpp/source",
        r#"
#include "{{ name | pascal_case }}.hpp"

{{ name | pascal_case }}::{{ name | pascal_case }}() {
    // Constructor logic
}

{{ name | pascal_case }}::~{{ name | pascal_case }}() {
    // Destructor logic
}

void {{ name | pascal_case }}::init() {
    std::cout << "Initializing {{ name }}" << std::endl;
}

void {{ name | pascal_case }}::step() {
    // Cyclic execution
}
"#,
    )
    .unwrap();

    // --- TYPESCRIPT ---
    tera.add_raw_template(
        "ts/class",
        r#"
/**
 * GÉNÉRÉ PAR RAISE
 * {{ description }}
 */
export class {{ name | pascal_case }} {
    public id: string = "{{ id }}";

    constructor() {
        console.log("{{ name }} initialized");
    }

    public execute(): void {
        // TODO: Implement logic
    }
}
"#,
    )
    .unwrap();
}

mod filters {
    use super::*;
    use heck::{ToLowerCamelCase, ToPascalCase, ToShoutySnakeCase, ToSnakeCase};

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

#[cfg(test)]
mod tests {
    use super::*;
    use tera::Context;

    #[test]
    fn test_cpp_rendering() {
        let engine = TemplateEngine::new();
        let mut ctx = Context::new();
        ctx.insert("name", "MotorController");
        ctx.insert("id", "M_01");

        let header = engine.render("cpp/header", &ctx).unwrap();
        assert!(header.contains("class MotorController"));
        assert!(header.contains("#pragma once"));

        let source = engine.render("cpp/source", &ctx).unwrap();
        assert!(source.contains("MotorController::init()"));
    }

    #[test]
    fn test_ts_rendering() {
        let engine = TemplateEngine::new();
        let mut ctx = Context::new();
        ctx.insert("name", "DashboardWidget");
        ctx.insert("id", "W_01");
        ctx.insert("description", "A widget");

        let ts = engine.render("ts/class", &ctx).unwrap();
        assert!(ts.contains("export class DashboardWidget"));
        assert!(ts.contains("public id: string"));
    }
}
