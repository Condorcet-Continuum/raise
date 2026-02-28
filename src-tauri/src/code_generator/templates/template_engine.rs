// FICHIER : src-tauri/src/code_generator/templates/template_engine.rs

use crate::utils::{
    data::{HashMap, Value},
    io::{Path, ProjectScope},
    prelude::*, // Importe RaiseResult et json!
};
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

    pub fn render(&self, template_name: &str, context: &Value) -> RaiseResult<String> {
        // 🎯 Magie du `?` : tera::Error est automatiquement converti en AppError
        // via les implémentations From dans utils/error.rs
        let tera_ctx = tera::Context::from_value(context.clone())?;
        let content = self.tera.render(template_name, &tera_ctx)?;

        Ok(content)
    }

    /// 🚀 GÉNÉRATION PHYSIQUE SÉCURISÉE
    pub async fn generate(
        &self,
        scope: &ProjectScope,
        template_name: &str,
        context: &Value,
        relative_path: impl AsRef<Path>,
    ) -> RaiseResult<()> {
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

    pub fn add_raw_template(&mut self, name: &str, content: &str) -> RaiseResult<()> {
        self.tera.add_raw_template(name, content)?;
        Ok(())
    }
}

// ⚠️ Note : tera::Result est conservé ici car l'interface Filter de Tera l'exige.
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
    // Les templates restent inchangés
    tera.add_raw_template("rust/actor", r#"..."#).unwrap();
    tera.add_raw_template("cpp/header", r#"..."#).unwrap();
    tera.add_raw_template("cpp/source", r#"..."#).unwrap();
    tera.add_raw_template("ts/class", r#"..."#).unwrap();
    tera.add_raw_template("verilog/module", r#"..."#).unwrap();
    tera.add_raw_template("vhdl/entity", r#"..."#).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::tempdir;

    #[tokio::test]
    async fn test_secure_generation() {
        let engine = TemplateEngine::new();
        let dir = tempdir().unwrap();
        let scope = ProjectScope::new(dir.path()).unwrap();

        // 🎯 MIGRATION V1.3 : Utilisation de json! au lieu de ContextBuilder
        let ctx = json!({
            "name": "SecureActor",
            "id": "SA_007"
        });

        let res = engine
            .generate(&scope, "rust/actor", &ctx, "src/actors/secure_actor.rs")
            .await;

        assert!(res.is_ok());

        let file_path = dir.path().join("src/actors/secure_actor.rs");
        assert!(file_path.exists());
    }
}
