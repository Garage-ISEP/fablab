#![allow(clippy::unwrap_used)]

use fablab::domain::errors::DomainError;
use fablab::domain::order::{Order, OrderStatus};
use fablab::domain::stock;
use fablab::domain::user::CasUser;

fn sample_order(material_id: Option<i64>, status: OrderStatus) -> Order
{
    Order
    {
        id: 1,
        user_id: 1,
        created_at: "2026-04-13T00:00:00".to_owned(),
        software_used: "Cura".to_owned(),
        material_id,
        quantity: 1,
        comments: None,
        status,
        requires_payment: false,
        sliced_weight_grams: None,
        print_time_minutes: None,
    }
}

#[test]
fn a_traiter_can_transition_to_en_traitement()
{
    assert!(OrderStatus::ATraiter.transition_to(OrderStatus::EnTraitement).is_ok());
}

#[test]
fn a_traiter_can_transition_to_annule()
{
    assert!(OrderStatus::ATraiter.transition_to(OrderStatus::Annule).is_ok());
}

#[test]
fn a_traiter_cannot_transition_to_imprime()
{
    assert!(OrderStatus::ATraiter.transition_to(OrderStatus::Imprime).is_err());
}

#[test]
fn a_traiter_cannot_transition_to_livre()
{
    assert!(OrderStatus::ATraiter.transition_to(OrderStatus::Livre).is_err());
}

#[test]
fn en_traitement_can_transition_to_imprime()
{
    assert!(OrderStatus::EnTraitement.transition_to(OrderStatus::Imprime).is_ok());
}

#[test]
fn en_traitement_can_transition_to_annule()
{
    assert!(OrderStatus::EnTraitement.transition_to(OrderStatus::Annule).is_ok());
}

#[test]
fn en_traitement_cannot_transition_to_livre()
{
    assert!(OrderStatus::EnTraitement.transition_to(OrderStatus::Livre).is_err());
}

#[test]
fn imprime_can_transition_to_livre()
{
    assert!(OrderStatus::Imprime.transition_to(OrderStatus::Livre).is_ok());
}

#[test]
fn imprime_can_transition_to_annule()
{
    assert!(OrderStatus::Imprime.transition_to(OrderStatus::Annule).is_ok());
}

#[test]
fn livre_cannot_transition_anywhere()
{
    assert!(OrderStatus::Livre.transition_to(OrderStatus::ATraiter).is_err());
    assert!(OrderStatus::Livre.transition_to(OrderStatus::EnTraitement).is_err());
    assert!(OrderStatus::Livre.transition_to(OrderStatus::Imprime).is_err());
    assert!(OrderStatus::Livre.transition_to(OrderStatus::Annule).is_err());
}

#[test]
fn annule_cannot_transition_anywhere()
{
    assert!(OrderStatus::Annule.transition_to(OrderStatus::ATraiter).is_err());
    assert!(OrderStatus::Annule.transition_to(OrderStatus::EnTraitement).is_err());
    assert!(OrderStatus::Annule.transition_to(OrderStatus::Imprime).is_err());
    assert!(OrderStatus::Annule.transition_to(OrderStatus::Livre).is_err());
}

#[test]
fn self_transition_is_allowed()
{
    assert!(OrderStatus::ATraiter.transition_to(OrderStatus::ATraiter).is_ok());
    assert!(OrderStatus::Livre.transition_to(OrderStatus::Livre).is_ok());
    assert!(OrderStatus::Annule.transition_to(OrderStatus::Annule).is_ok());
}

#[test]
fn order_status_roundtrip_from_str()
{
    for s in &["a_traiter", "en_traitement", "imprime", "livre", "annule"]
    {
        let parsed: OrderStatus = s.parse().unwrap();
        assert_eq!(parsed.as_str(), *s);
    }
}

#[test]
fn invalid_status_string_returns_error()
{
    let result: Result<OrderStatus, _> = "invalid".parse();
    assert!(result.is_err());
}

#[test]
fn cas_user_fields()
{
    let user = CasUser
    {
        cas_login: "situ62394".to_owned(),
        display_name: "Simon TULOUP".to_owned(),
        email: "situ62394@eleve.isep.fr".to_owned(),
        promo: Some("Paris-ING-A2-2526".to_owned()),
    };
    assert_eq!(user.cas_login, "situ62394");
    assert_eq!(user.promo.as_deref(), Some("Paris-ING-A2-2526"));
}

// ============================================================
// Order::try_advance_status — material invariant
// ============================================================

#[test]
fn try_advance_rejects_en_traitement_without_material()
{
    let order = sample_order(None, OrderStatus::ATraiter);
    let err = order.try_advance_status(OrderStatus::EnTraitement).unwrap_err();
    assert!(matches!(err, DomainError::MaterialRequiredForStatus { .. }));
}

#[test]
fn try_advance_rejects_imprime_without_material()
{
    let order = sample_order(None, OrderStatus::EnTraitement);
    let err = order.try_advance_status(OrderStatus::Imprime).unwrap_err();
    assert!(matches!(err, DomainError::MaterialRequiredForStatus { .. }));
}

#[test]
fn try_advance_rejects_livre_without_material()
{
    let order = sample_order(None, OrderStatus::Imprime);
    let err = order.try_advance_status(OrderStatus::Livre).unwrap_err();
    assert!(matches!(err, DomainError::MaterialRequiredForStatus { .. }));
}

#[test]
fn try_advance_allows_annule_without_material()
{
    let order = sample_order(None, OrderStatus::ATraiter);
    assert!(order.try_advance_status(OrderStatus::Annule).is_ok());
}

#[test]
fn try_advance_allows_normal_transitions_when_material_set()
{
    let order = sample_order(Some(7), OrderStatus::ATraiter);
    assert!(order.try_advance_status(OrderStatus::EnTraitement).is_ok());
}

#[test]
fn try_advance_still_blocks_invalid_transitions_when_material_set()
{
    let order = sample_order(Some(7), OrderStatus::ATraiter);
    let err = order.try_advance_status(OrderStatus::Livre).unwrap_err();
    assert!(matches!(err, DomainError::InvalidStatusTransition { .. }));
}

// ============================================================
// stock module — via the public domain API
// ============================================================

#[test]
fn stock_remaining_weight_handles_typical_case()
{
    assert!((stock::remaining_weight(1000.0, 250.0) - 750.0).abs() < f64::EPSILON);
}

#[test]
fn stock_check_sufficient_accepts_exact_fit()
{
    assert!(stock::check_sufficient(1, 1000.0, 700.0, 300.0).is_ok());
}

#[test]
fn stock_check_sufficient_rejects_overdraft()
{
    let err = stock::check_sufficient(9, 500.0, 400.0, 200.0).unwrap_err();
    match err
    {
        DomainError::InsufficientStock { material_id, available_grams, requested_grams } =>
        {
            assert_eq!(material_id, 9);
            assert!((available_grams - 100.0).abs() < f64::EPSILON);
            assert!((requested_grams - 200.0).abs() < f64::EPSILON);
        }
        other => panic!("expected InsufficientStock, got {other:?}"),
    }
}
