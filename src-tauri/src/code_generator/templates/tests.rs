use tera::{Context, Tera};

#[test]
fn test_tera_engine_basic_rendering() {
    // Ce test valide que la librairie de templating est bien intégrée
    // et capable de faire des substitutions basiques.

    let mut tera = Tera::default();
    tera.add_raw_template("hello", "Hello {{ name }}!").unwrap();

    let mut context = Context::new();
    context.insert("name", "GenAptitude");

    let result = tera.render("hello", &context);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello GenAptitude!");
}

#[test]
fn test_tera_filters() {
    // Vérifie qu'on peut utiliser des filtres (utile pour passer de "Nom Acteur" à "NomActeur")
    let mut tera = Tera::default();
    tera.add_raw_template("filter", "{{ text | replace(from=' ', to='') }}")
        .unwrap();

    let mut context = Context::new();
    context.insert("text", "Nom Avec Espaces");

    let result = tera.render("filter", &context).unwrap();
    assert_eq!(result, "NomAvecEspaces");
}
