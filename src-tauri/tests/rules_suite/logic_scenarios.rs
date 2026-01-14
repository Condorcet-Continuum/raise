// FICHIER : src-tauri/tests/rules_suite/logic_scenarios.rs

use raise::rules_engine::{Evaluator, Expr, NoOpDataProvider};
use serde_json::json;

#[test]
fn test_complex_boolean_logic() {
    let rule = Expr::Or(vec![
        Expr::And(vec![
            Expr::Gt(
                Box::new(Expr::Var("age".into())),
                Box::new(Expr::Val(json!(18))),
            ),
            Expr::Eq(
                Box::new(Expr::Var("status".into())),
                Box::new(Expr::Val(json!("member"))),
            ),
        ]),
        Expr::Eq(
            Box::new(Expr::Var("role".into())),
            Box::new(Expr::Val(json!("admin"))),
        ),
    ]);

    let provider = NoOpDataProvider;

    let ctx1 = json!({ "age": 16, "status": "member", "role": "user" });
    // CORRECTION : .into_owned()
    assert_eq!(
        Evaluator::evaluate(&rule, &ctx1, &provider)
            .unwrap()
            .into_owned(),
        json!(false)
    );

    let ctx3 = json!({ "age": 25, "status": "member", "role": "user" });
    assert_eq!(
        Evaluator::evaluate(&rule, &ctx3, &provider)
            .unwrap()
            .into_owned(),
        json!(true)
    );

    let ctx4 = json!({ "age": 10, "status": "guest", "role": "admin" });
    assert_eq!(
        Evaluator::evaluate(&rule, &ctx4, &provider)
            .unwrap()
            .into_owned(),
        json!(true)
    );
}

#[test]
fn test_math_precedence() {
    let rule = Expr::Div(vec![
        Expr::Sub(vec![Expr::Var("price".into()), Expr::Var("cost".into())]),
        Expr::Var("price".into()),
    ]);

    let provider = NoOpDataProvider;
    let ctx = json!({ "price": 100.0, "cost": 75.0 });

    let res = Evaluator::evaluate(&rule, &ctx, &provider).unwrap();
    // CORRECTION : Comparaison typée
    assert_eq!(res.as_f64(), Some(0.25));
}
