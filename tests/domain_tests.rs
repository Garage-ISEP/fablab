#![allow(clippy::unwrap_used)]

use fablab::domain::order::OrderStatus;
use fablab::domain::user::CasUser;

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
