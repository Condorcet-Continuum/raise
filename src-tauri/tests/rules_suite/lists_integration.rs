// FICHIER : src-tauri/tests/rules_suite/lists_integration.rs

use raise::rules_engine::{Evaluator, Expr, NoOpDataProvider};
use serde_json::json;

/// Teste la fonction Len() sur des tableaux et chaînes
#[test]
fn test_len_operator() {
    let provider = NoOpDataProvider;

    let ctx = json!({
        "tags": ["a", "b", "c"],
        "title": "Hello World"
    });

    // Len(tags) -> 3
    let rule_arr = Expr::Len(Box::new(Expr::Var("tags".into())));
    assert_eq!(
        Evaluator::evaluate(&rule_arr, &ctx, &provider)
            .unwrap()
            .as_i64(),
        Some(3)
    );

    // Len(title) -> 11
    let rule_str = Expr::Len(Box::new(Expr::Var("title".into())));
    assert_eq!(
        Evaluator::evaluate(&rule_str, &ctx, &provider)
            .unwrap()
            .as_i64(),
        Some(11)
    );
}

/// Teste Map() : Transformation d'un tableau d'objets
#[test]
fn test_map_transformation() {
    let provider = NoOpDataProvider;

    let ctx = json!({
        "order_lines": [
            { "price": 10, "qty": 2 },
            { "price": 20, "qty": 1 }
        ]
    });

    // map(order_lines, "line", line.price * line.qty)
    let rule = Expr::Map {
        list: Box::new(Expr::Var("order_lines".into())),
        alias: "line".into(),
        expr: Box::new(Expr::Mul(vec![
            Expr::Var("line.price".into()),
            Expr::Var("line.qty".into()),
        ])),
    };

    let res = Evaluator::evaluate(&rule, &ctx, &provider).unwrap();
    let arr = res.as_array().unwrap();

    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].as_i64(), Some(20));
    assert_eq!(arr[1].as_i64(), Some(20));
}

/// Teste Filter() avec contexte global
#[test]
fn test_filter_context() {
    let provider = NoOpDataProvider;
    let ctx = json!({
        "limit": 50,
        "values": [10, 60, 20, 90, 50]
    });

    // filter(values, "v", v >= limit)
    let rule = Expr::Filter {
        list: Box::new(Expr::Var("values".into())),
        alias: "v".into(),
        condition: Box::new(Expr::Gte(
            Box::new(Expr::Var("v".into())),
            Box::new(Expr::Var("limit".into())),
        )),
    };

    let res = Evaluator::evaluate(&rule, &ctx, &provider).unwrap();
    let arr = res.as_array().unwrap();

    assert_eq!(arr.len(), 3); // 60, 90, 50
}
