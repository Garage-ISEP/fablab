#![allow(clippy::unwrap_used, clippy::expect_used)]

use fablab::infrastructure::db::admin_repo::SqliteAdminRepository;
use fablab::infrastructure::db::material_repo::SqliteMaterialRepository;
use fablab::infrastructure::db::migrations::run_migrations;
use fablab::infrastructure::db::order_repo::SqliteOrderRepository;
use fablab::infrastructure::db::pool::DbPool;
use fablab::infrastructure::db::user_repo::SqliteUserRepository;

use fablab::domain::errors::DomainError;
use fablab::domain::material::Material;
use fablab::domain::order::{NewOrder, OrderStatus};
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
        spool_weight_grams: 1000.0,
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
            available: true,
            spool_weight_grams: 1000.0,
        }
    ).unwrap();

    repo.upsert
    (
        &Material
        {
            id: 2,
            name: "ABS".to_owned(),
            color: "Blanc".to_owned(),
            available: false,
            spool_weight_grams: 1000.0,
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

// ============================================================
// Stock-related tests
// ============================================================

fn seed_stock_fixture(pool: &DbPool) -> (i64, i64)
{
    let user_repo = SqliteUserRepository::new(pool.clone());
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool.clone());

    let user = user_repo.upsert_from_cas(&CasUser
    {
        cas_login: "stock_user".to_owned(),
        display_name: "Stock Tester".to_owned(),
        email: "s@isep.fr".to_owned(),
        promo: None,
    }).unwrap();

    material_repo.upsert(&Material
    {
        id: 1,
        name: "PLA".to_owned(),
        color: "Noir".to_owned(),
        available: true,
        spool_weight_grams: 1000.0,
    }).unwrap();

    let o1 = order_repo.create(NewOrder
    {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: Some(1),
        quantity: 1,
        comments: None,
    }).unwrap();
    let mut o1_mut = order_repo.find_by_id(o1.id).unwrap().unwrap();
    o1_mut.sliced_weight_grams = Some(200.0);
    order_repo.update(&o1_mut).unwrap();

    let o2 = order_repo.create(NewOrder
    {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: Some(1),
        quantity: 1,
        comments: None,
    }).unwrap();
    let mut o2_mut = order_repo.find_by_id(o2.id).unwrap().unwrap();
    o2_mut.sliced_weight_grams = Some(300.0);
    o2_mut.status = OrderStatus::Annule;
    order_repo.update(&o2_mut).unwrap();

    let o3 = order_repo.create(NewOrder
    {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: Some(1),
        quantity: 1,
        comments: None,
    }).unwrap();
    let mut o3_mut = order_repo.find_by_id(o3.id).unwrap().unwrap();
    o3_mut.sliced_weight_grams = Some(150.0);
    order_repo.update(&o3_mut).unwrap();

    (user.id, o3.id)
}

#[test]
fn sum_weight_excludes_cancelled_orders()
{
    let pool = setup_pool();
    let (_user_id, _) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let total = order_repo.sum_weight_by_material(1, None).unwrap();
    assert!((total - 350.0).abs() < f64::EPSILON);
}

#[test]
fn sum_weight_respects_exclude_order_id()
{
    let pool = setup_pool();
    let (_user_id, o3_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let total = order_repo.sum_weight_by_material(1, Some(o3_id)).unwrap();
    assert!((total - 200.0).abs() < f64::EPSILON);
}

#[test]
fn sum_weight_returns_zero_for_material_without_orders()
{
    let pool = setup_pool();
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    material_repo.upsert(&Material
    {
        id: 42,
        name: "PETG".to_owned(),
        color: "Transparent".to_owned(),
        available: true,
        spool_weight_grams: 1000.0,
    }).unwrap();

    let total = order_repo.sum_weight_by_material(42, None).unwrap();
    assert_eq!(total, 0.0);
}

#[test]
fn update_if_stock_sufficient_commits_when_within_spool()
{
    let pool = setup_pool();
    let (_user_id, o3_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let mut o3 = order_repo.find_by_id(o3_id).unwrap().unwrap();
    o3.sliced_weight_grams = Some(500.0);
    order_repo.update_if_stock_sufficient(&o3, 1000.0).unwrap();

    let reloaded = order_repo.find_by_id(o3_id).unwrap().unwrap();
    assert_eq!(reloaded.sliced_weight_grams, Some(500.0));
}

#[test]
fn update_if_stock_sufficient_rolls_back_on_overdraft()
{
    let pool = setup_pool();
    let (_user_id, o3_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let mut o3 = order_repo.find_by_id(o3_id).unwrap().unwrap();
    o3.sliced_weight_grams = Some(900.0);
    let err = order_repo.update_if_stock_sufficient(&o3, 1000.0).unwrap_err();

    assert!(matches!(err, DomainError::InsufficientStock { .. }));

    let reloaded = order_repo.find_by_id(o3_id).unwrap().unwrap();
    assert_eq!(reloaded.sliced_weight_grams, Some(150.0));
}

#[test]
fn update_persists_material_id_change()
{
    let pool = setup_pool();
    let user_repo = SqliteUserRepository::new(pool.clone());
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    let user = user_repo.upsert_from_cas(&CasUser
    {
        cas_login: "swap_test".to_owned(),
        display_name: "Swap".to_owned(),
        email: "swap@isep.fr".to_owned(),
        promo: None,
    }).unwrap();

    material_repo.upsert(&Material
    {
        id: 1, name: "PLA".to_owned(), color: "Noir".to_owned(),
        available: true, spool_weight_grams: 1000.0,
    }).unwrap();
    material_repo.upsert(&Material
    {
        id: 2, name: "ABS".to_owned(), color: "Blanc".to_owned(),
        available: true, spool_weight_grams: 1000.0,
    }).unwrap();

    let created = order_repo.create(NewOrder
    {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: Some(1),
        quantity: 1,
        comments: None,
    }).unwrap();

    let mut o = order_repo.find_by_id(created.id).unwrap().unwrap();
    o.material_id = Some(2);
    order_repo.update(&o).unwrap();

    let reloaded = order_repo.find_by_id(created.id).unwrap().unwrap();
    assert_eq!(reloaded.material_id, Some(2));
}
