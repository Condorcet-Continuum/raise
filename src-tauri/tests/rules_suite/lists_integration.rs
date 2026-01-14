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

    // map(order_lines, "line", line.price * line.qty) -> [20, 20]
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
    assert_eq!(arr[0].as_f64(), Some(20.0));
    assert_eq!(arr[1].as_f64(), Some(20.0));
}

/// Teste Filter() : Sélection selon critère
#[test]
fn test_filter_selection() {
    let provider = NoOpDataProvider;

    let ctx = json!({
        "scores": [10, 55, 80, 45, 90]
    });

    // filter(scores, "s", s > 50) -> [55, 80, 90]
    let rule = Expr::Filter {
        list: Box::new(Expr::Var("scores".into())),
        alias: "s".into(),
        condition: Box::new(Expr::Gt(
            Box::new(Expr::Var("s".into())),
            Box::new(Expr::Val(json!(50))),
        )),
    };

    let res = Evaluator::evaluate(&rule, &ctx, &provider).unwrap();
    let arr = res.as_array().unwrap();

    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_i64(), Some(55));
    assert_eq!(arr[2].as_i64(), Some(90));
}

/// Teste la composition Map(Filter(...)) et l'accès au scope global
#[test]
fn test_chained_operations_with_global_context() {
    let provider = NoOpDataProvider;

    let ctx = json!({
        "min_age": 18,
        "users": [
            { "name": "Alice", "age": 25 },
            { "name": "Bob", "age": 15 },
            { "name": "Charlie", "age": 30 }
        ]
    });

    // 1. Filter: Garder users où user.age >= min_age (global var)
    let filtered_users = Expr::Filter {
        list: Box::new(Expr::Var("users".into())),
        alias: "u".into(),
        condition: Box::new(Expr::Gte(
            Box::new(Expr::Var("u.age".into())),
            Box::new(Expr::Var("min_age".into())), // Accès global depuis scope local
        )),
    };

    // 2. Map: Extraire les noms en majuscules
    let rule = Expr::Map {
        list: Box::new(filtered_users),
        alias: "u_valid".into(),
        expr: Box::new(Expr::Upper(Box::new(Expr::Var("u_valid.name".into())))),
    };

    let res = Evaluator::evaluate(&rule, &ctx, &provider).unwrap();
    let arr = res.as_array().unwrap();

    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].as_str(), Some("ALICE"));
    assert_eq!(arr[1].as_str(), Some("CHARLIE"));
    // Bob est exclu car 15 < 18
}
