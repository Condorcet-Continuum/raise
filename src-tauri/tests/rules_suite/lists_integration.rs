// FICHIER : src-tauri/tests/rules_suite/lists_integration.rs

use raise::rules_engine::{Evaluator, Expr, NoOpDataProvider};
use serde_json::json;

/// Teste la fonction Len() sur des tableaux et chaînes
#[tokio::test]
async fn test_len_operator() {
    let provider = NoOpDataProvider;

    let ctx = json!({
        "tags": ["a", "b", "c"],
        "title": "Hello World"
    });

    // Len(tags) -> 3
    let rule_arr = Expr::Len(Box::new(Expr::Var("tags".into())));

    // CORRECTIF : .into_owned() transforme le Cow<Value> en Value pour matcher json!(3)
    let res_arr = Evaluator::evaluate(&rule_arr, &ctx, &provider)
        .await
        .expect("Evaluation failed for tags")
        .into_owned();

    assert_eq!(res_arr, json!(3), "Len(tags) devrait valoir 3");

    // Len(title) -> 11
    let rule_str = Expr::Len(Box::new(Expr::Var("title".into())));

    // CORRECTIF : .into_owned() ici aussi
    let res_str = Evaluator::evaluate(&rule_str, &ctx, &provider)
        .await
        .expect("Evaluation failed for title")
        .into_owned();

    assert_eq!(res_str, json!(11), "Len(title) devrait valoir 11");
}

/// Teste Map() : Transformation d'un tableau d'objets
#[tokio::test]
async fn test_map_transformation() {
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

    let res = Evaluator::evaluate(&rule, &ctx, &provider).await.unwrap();
    let arr = res.as_array().expect("Le résultat doit être un tableau");

    assert_eq!(arr.len(), 2);
    // Comparaison avec référence pour éviter tout souci de type
    assert_eq!(&arr[0], &json!(20));
    assert_eq!(&arr[1], &json!(20));
}

/// Teste Filter() avec contexte global
#[tokio::test]
async fn test_filter_context() {
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

    let res = Evaluator::evaluate(&rule, &ctx, &provider).await.unwrap();
    let arr = res.as_array().expect("Le résultat doit être un tableau");

    // 60, 90, 50 (>= 50)
    assert_eq!(arr.len(), 3);
    assert!(arr.contains(&json!(60)));
    assert!(arr.contains(&json!(90)));
    assert!(arr.contains(&json!(50)));
}
