// FICHIER : src-tauri/tests/rules_suite/stdlib_tests.rs

use raise::rules_engine::{Evaluator, Expr, NoOpDataProvider};
use serde_json::json;

#[test]
fn test_math_extensions() {
    let provider = NoOpDataProvider;
    let ctx = json!({});

    // Abs
    let abs = Expr::Abs(Box::new(Expr::Val(json!(-42))));
    assert_eq!(
        Evaluator::evaluate(&abs, &ctx, &provider).unwrap().as_f64(),
        Some(42.0)
    );

    // Round
    let r1 = Expr::Round {
        value: Box::new(Expr::Val(json!(3.14159))),
        precision: Box::new(Expr::Val(json!(2))),
    };
    assert_eq!(
        Evaluator::evaluate(&r1, &ctx, &provider).unwrap().as_f64(),
        Some(3.14)
    );
}

#[test]
fn test_string_extensions() {
    let provider = NoOpDataProvider;
    let ctx = json!({ "msg": "  Bonjour Monde  " });

    // Trim + Lower
    let expr = Expr::Lower(Box::new(Expr::Trim(Box::new(Expr::Var("msg".into())))));
    assert_eq!(
        Evaluator::evaluate(&expr, &ctx, &provider)
            .unwrap()
            .as_str(),
        Some("bonjour monde")
    );

    // Replace
    let repl = Expr::Replace {
        value: Box::new(Expr::Var("msg".into())),
        pattern: Box::new(Expr::Val(json!("Monde"))),
        replacement: Box::new(Expr::Val(json!("Raise"))),
    };
    assert_eq!(
        Evaluator::evaluate(&repl, &ctx, &provider)
            .unwrap()
            .as_str(),
        Some("  Bonjour Raise  ")
    );
}

#[test]
fn test_list_aggregations() {
    let provider = NoOpDataProvider;
    let ctx = json!({ "values": [10, 2, 50, 5] });

    let min = Expr::Min(Box::new(Expr::Var("values".into())));
    let max = Expr::Max(Box::new(Expr::Var("values".into())));

    assert_eq!(
        Evaluator::evaluate(&min, &ctx, &provider).unwrap().as_f64(),
        Some(2.0)
    );
    assert_eq!(
        Evaluator::evaluate(&max, &ctx, &provider).unwrap().as_f64(),
        Some(50.0)
    );
}
