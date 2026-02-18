// FICHIER : src-tauri/tests/rules_suite/logic_scenarios.rs

use raise::rules_engine::ast::Expr;
use raise::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use raise::utils::prelude::*;

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_complex_access_control() {
    // Scénario : L'utilisateur a accès SI :
    // (status == "member" ET role == "admin")

    let rule = Expr::And(vec![
        // CORRECTION : Eq prend maintenant un vecteur
        Expr::Eq(vec![
            Expr::Var("status".to_string()),
            Expr::Val(json!("member")),
        ]),
        Expr::Eq(vec![
            Expr::Var("role".to_string()),
            Expr::Val(json!("admin")),
        ]),
    ]);

    let provider = NoOpDataProvider;

    // Cas 1 : Succès
    let ctx_admin = json!({
        "status": "member",
        "role": "admin"
    });
    // CORRECTION E0599 : Ajout de .await car l'évaluateur est asynchrone
    let result_ok = Evaluator::evaluate(&rule, &ctx_admin, &provider)
        .await
        .expect("Evaluation failed");
    assert_eq!(result_ok.as_bool(), Some(true));

    // Cas 2 : Echec (Mauvais statut)
    let ctx_guest = json!({
        "status": "guest",
        "role": "admin"
    });
    // CORRECTION E0599 : Ajout de .await
    let result_fail = Evaluator::evaluate(&rule, &ctx_guest, &provider)
        .await
        .expect("Evaluation failed");
    assert_eq!(result_fail.as_bool(), Some(false));
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_nested_logic_with_values() {
    // Scénario : (A > 10) OU (B == 0)
    let rule = Expr::Or(vec![
        // Gt reste binaire (Box, Box) car défini ainsi dans ast.rs
        Expr::Gt(
            Box::new(Expr::Var("a".to_string())),
            Box::new(Expr::Val(json!(10))),
        ),
        // Eq est n-aire (Vec)
        Expr::Eq(vec![Expr::Var("b".to_string()), Expr::Val(json!(0))]),
    ]);

    let provider = NoOpDataProvider;

    // a=5, b=0 -> True (grâce au OR)
    let ctx = json!({ "a": 5, "b": 0 });
    // CORRECTION E0599 : Ajout de .await
    let res = Evaluator::evaluate(&rule, &ctx, &provider).await.unwrap();
    assert_eq!(res.as_bool(), Some(true));
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_complex_boolean_logic() {
    let rule = Expr::Or(vec![
        Expr::And(vec![
            Expr::Gt(
                Box::new(Expr::Var("age".into())),
                Box::new(Expr::Val(json!(18))),
            ),
            Expr::Eq(vec![Expr::Var("status".into()), Expr::Val(json!("member"))]),
        ]),
        Expr::Eq(vec![Expr::Var("role".into()), Expr::Val(json!("admin"))]),
    ]);

    let provider = NoOpDataProvider;

    let ctx1 = json!({ "age": 16, "status": "member", "role": "user" });
    assert_eq!(
        // CORRECTION E0599 : Ajout de .await
        Evaluator::evaluate(&rule, &ctx1, &provider)
            .await
            .unwrap()
            .into_owned(),
        json!(false)
    );

    let ctx3 = json!({ "age": 25, "status": "member", "role": "user" });
    assert_eq!(
        // CORRECTION E0599 : Ajout de .await
        Evaluator::evaluate(&rule, &ctx3, &provider)
            .await
            .unwrap()
            .into_owned(),
        json!(true)
    );

    let ctx4 = json!({ "age": 10, "status": "guest", "role": "admin" });
    assert_eq!(
        // CORRECTION E0599 : Ajout de .await
        Evaluator::evaluate(&rule, &ctx4, &provider)
            .await
            .unwrap()
            .into_owned(),
        json!(true)
    );
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_math_precedence() {
    // (price - cost) / price
    let rule = Expr::Div(vec![
        Expr::Sub(vec![Expr::Var("price".into()), Expr::Var("cost".into())]),
        Expr::Var("price".into()),
    ]);

    let ctx = json!({ "price": 100.0, "cost": 75.0 });
    let provider = NoOpDataProvider;

    assert_eq!(
        // CORRECTION E0599 : Ajout de .await
        Evaluator::evaluate(&rule, &ctx, &provider)
            .await
            .unwrap()
            .as_f64(),
        Some(0.25)
    );
}
