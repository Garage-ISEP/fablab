#![allow(clippy::unwrap_used, clippy::expect_used)]

use fablab::infrastructure::db::admin_repo::SqliteAdminRepository;
use fablab::infrastructure::db::material_repo::SqliteMaterialRepository;
use fablab::infrastructure::db::migrations::run_migrations;
use fablab::infrastructure::db::order_repo::SqliteOrderRepository;
use fablab::infrastructure::db::pool::DbPool;
use fablab::infrastructure::db::user_repo::SqliteUserRepository;

use fablab::domain::material::Material;
use fablab::domain::order::NewOrder;
use fablab::domain::repositories::
{
    AdminRepository, MaterialRepository, OrderRepository, UserRepository,
};
use fablab::domain::user::CasUser;

fn setup_pool() -> DbPool
{
    let pool = DbPool::open_in_memory().expect("failed to open in-memory db");
    run_migrations(&pool).expect("failed to run migrations");
    pool
}

#[test]
fn upsert_from_cas_creates_then_finds()
{
    let pool = setup_pool();
    let repo = SqliteUserRepository::new(pool);
    let cas = CasUser
    {
        cas_login: "situ62394".to_owned(),
        display_name: "Simon TULOUP".to_owned(),
        email: "situ62394@eleve.isep.fr".to_owned(),
        promo: Some("A2".to_owned()),
    };
    let user = repo.upsert_from_cas(&cas).expect("upsert failed");
    assert_eq!(user.cas_login, "situ62394");

    let found = repo.find_by_cas_login("situ62394")
        .expect("find failed")
        .expect("not found");
    assert_eq!(found.id, user.id);
}

#[test]
fn create_order_then_find_by_user()
{
    let pool = setup_pool();
    let user_repo = SqliteUserRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    let cas = CasUser
    {
        cas_login: "order_test".to_owned(),
        display_name: "Test".to_owned(),
        email: "t@isep.fr".to_owned(),
        promo: None,
    };
    let user = user_repo.upsert_from_cas(&cas).expect("upsert failed");

    let new_order = NewOrder
    {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: None,
        quantity: 2,
        comments: None,
    };
    let created = order_repo.create(new_order).expect("create failed");
    assert_eq!(created.status.as_str(), "a_traiter");
    assert!(!created.requires_payment);

    let orders = order_repo.find_by_user(user.id).expect("find_by_user failed");
    assert_eq!(orders.len(), 1);
}

#[test]
fn material_max_id()
{
    let pool = setup_pool();
    let repo = SqliteMaterialRepository::new(pool);

    assert_eq!(repo.max_id().unwrap(), 0);

    repo.upsert(&Material
    {
        id: 5,
        name: "PLA".to_owned(),
        color: "Noir".to_owned(),
        available: true,
    })
    .unwrap();

    assert_eq!(repo.max_id().unwrap(), 5);
}

#[test]
fn find_available_materials()
{
    let pool = setup_pool();
    let repo = SqliteMaterialRepository::new(pool);

    repo.upsert
    (
        &Material 
        { 
            id: 1, 
            name: "PLA".to_owned(), 
            color: "Rouge".to_owned(), 
            available: true 
        }
    ).unwrap();
    
    repo.upsert
    (
        &Material 
        { 
            id: 2, 
            name: "ABS".to_owned(), 
            color: "Blanc".to_owned(),
            available: false 
        }
    ).unwrap();

    let available = repo.find_available().unwrap();
    assert_eq!(available.len(), 1);
    assert!(available[0].available);
}

#[test]
fn admin_create_and_find()
{
    let pool = setup_pool();
    let repo = SqliteAdminRepository::new(pool);

    let admin = repo.create("admin", "$argon2id$fake_hash").expect("create failed");
    assert_eq!(admin.login, "admin");

    let found = repo.find_by_login("admin")
        .expect("find failed")
        .expect("not found");
    assert_eq!(found.id, admin.id);
}

#[test]
fn admin_find_missing_returns_none()
{
    let pool = setup_pool();
    let repo = SqliteAdminRepository::new(pool);
    assert!(repo.find_by_login("nobody").unwrap().is_none());
}
