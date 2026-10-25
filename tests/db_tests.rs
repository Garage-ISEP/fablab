#![allow(clippy::unwrap_used, clippy::expect_used)]

use fablab::infrastructure::db::admin_repo::SqliteAdminRepository;
use fablab::infrastructure::db::bambu_account_repo::SqliteBambuAccountRepository;
use fablab::infrastructure::db::material_repo::SqliteMaterialRepository;
use fablab::infrastructure::db::migrations::run_migrations;
use fablab::infrastructure::db::order_repo::SqliteOrderRepository;
use fablab::infrastructure::db::pool::DbPool;
use fablab::infrastructure::db::user_repo::SqliteUserRepository;

use fablab::domain::errors::DomainError;
use fablab::domain::filament::{BambuCredentials, PrinterDevice};
use fablab::domain::material::LoadedMaterial;
use fablab::domain::order::{NewOrder, OrderStatus};
use fablab::domain::repositories::{
    AdminRepository, BambuAccountRepository, MaterialRepository, OrderRepository, UserRepository,
};
use fablab::domain::user::CasUser;

fn setup_pool() -> DbPool {
    let pool = DbPool::open_in_memory().expect("failed to open in-memory db");
    run_migrations(&pool).expect("failed to run migrations");
    pool
}

#[test]
fn upsert_from_cas_creates_then_finds() {
    let pool = setup_pool();
    let repo = SqliteUserRepository::new(pool);
    let cas = CasUser {
        cas_login: "situ62394".to_owned(),
        display_name: "Simon TULOUP".to_owned(),
        email: "situ62394@eleve.isep.fr".to_owned(),
        promo: Some("A2".to_owned()),
    };
    let user = repo.upsert_from_cas(&cas).expect("upsert failed");
    assert_eq!(user.cas_login, "situ62394");

    let found = repo
        .find_by_cas_login("situ62394")
        .expect("find failed")
        .expect("not found");
    assert_eq!(found.id, user.id);
}

#[test]
fn create_order_then_find_by_user() {
    let pool = setup_pool();
    let user_repo = SqliteUserRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    let cas = CasUser {
        cas_login: "order_test".to_owned(),
        display_name: "Test".to_owned(),
        email: "t@isep.fr".to_owned(),
        promo: None,
    };
    let user = user_repo.upsert_from_cas(&cas).expect("upsert failed");

    let new_order = NewOrder {
        user_id: user.id,
        software_used: "Cura".to_owned(),
        material_id: None,
        quantity: 2,
        comments: None,
    };
    let created = order_repo.create(new_order).expect("create failed");
    assert_eq!(created.status.as_str(), "a_traiter");
    assert!(!created.requires_payment);

    let orders = order_repo
        .find_by_user(user.id)
        .expect("find_by_user failed");
    assert_eq!(orders.len(), 1);
}

fn loaded(key: &str, name: &str, color: &str) -> LoadedMaterial {
    LoadedMaterial {
        bambu_key: key.to_owned(),
        name: name.to_owned(),
        color: color.to_owned(),
        color_hex: "C12E1F".to_owned(),
    }
}

/// Creates the given products through an AMS sync and returns their ids.
fn seed_materials(repo: &SqliteMaterialRepository, materials: &[LoadedMaterial]) -> Vec<i64> {
    repo.sync_loaded(materials).unwrap();
    let all = repo.find_all().unwrap();
    materials
        .iter()
        .map(|m| {
            all.iter()
                .find(|row| row.bambu_key.as_deref() == Some(m.bambu_key.as_str()))
                .unwrap()
                .id
        })
        .collect()
}

#[test]
fn sync_loaded_creates_materials_and_tracks_availability() {
    let pool = setup_pool();
    let repo = SqliteMaterialRepository::new(pool);

    let ids = seed_materials(
        &repo,
        &[
            loaded("GFA00:C12E1F", "PLA Basic", "Rouge"),
            loaded("GFG02:000000", "PETG HF", "Noir"),
        ],
    );
    assert_eq!(repo.find_available().unwrap().len(), 2);

    // The red spool is removed from the AMS: the row stays, unavailable.
    repo.sync_loaded(&[loaded("GFG02:000000", "PETG HF", "Noir")])
        .unwrap();
    let red = repo.find_by_id(ids[0]).unwrap().unwrap();
    assert!(!red.available);
    assert_eq!(red.color_hex.as_deref(), Some("C12E1F"));
    let available = repo.find_available().unwrap();
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, ids[1]);

    // Loaded again later: same id, available again.
    repo.sync_loaded(&[loaded("GFA00:C12E1F", "PLA Basic", "Rouge")])
        .unwrap();
    assert!(repo.find_by_id(ids[0]).unwrap().unwrap().available);
    assert_eq!(repo.find_all().unwrap().len(), 2);
}

#[test]
fn sync_loaded_adopts_a_legacy_material_with_the_same_label() {
    let pool = setup_pool();
    pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO materials (id, name, color, available) VALUES (7, 'PLA Basic', 'Rouge', 0)",
            [],
        )
        .map_err(|e| DomainError::Database(e.to_string()))
    })
    .unwrap();
    let repo = SqliteMaterialRepository::new(pool);

    repo.sync_loaded(&[loaded("GFA00:C12E1F", "PLA Basic", "Rouge")])
        .unwrap();

    let all = repo.find_all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, 7);
    assert_eq!(all[0].bambu_key.as_deref(), Some("GFA00:C12E1F"));
    assert!(all[0].available);
}

#[test]
fn migration_003_keeps_materials_and_their_orders() {
    let pool = DbPool::open_in_memory().unwrap();
    pool.with_conn(|conn| {
        let sql = [
            include_str!("../migrations/001_create_db.sql"),
            include_str!("../migrations/002_material_spool_weight.sql"),
            "CREATE TABLE _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')));
             INSERT INTO _migrations (name) VALUES ('001_create_db'), ('002_material_spool_weight');
             INSERT INTO users (id, cas_login, display_name, email) VALUES (1, 'u', 'U', 'u@isep.fr');
             INSERT INTO materials (id, name, color, available, spool_weight_grams) VALUES (3, 'PLA', 'Noir', 1, 750.0);
             INSERT INTO orders (user_id, software_used, material_id) VALUES (1, 'Cura', 3);",
        ];
        for batch in sql {
            conn.execute_batch(batch)
                .map_err(|e| DomainError::Database(e.to_string()))?;
        }
        Ok(())
    })
    .unwrap();

    run_migrations(&pool).unwrap();

    let material = SqliteMaterialRepository::new(pool.clone())
        .find_by_id(3)
        .unwrap()
        .unwrap();
    assert_eq!(material.label(), "PLA - Noir");
    assert!(!material.available);
    assert_eq!(material.bambu_key, None);

    let orders = SqliteOrderRepository::new(pool.clone())
        .find_by_user(1)
        .unwrap();
    assert_eq!(orders[0].material_id, Some(3));
    let violations: i64 = pool
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .map_err(|e| DomainError::Database(e.to_string()))
        })
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn bambu_account_save_replaces_the_previous_token() {
    let pool = setup_pool();
    let repo = SqliteBambuAccountRepository::new(pool);
    assert!(repo.load().unwrap().is_none());

    let devices = vec![PrinterDevice {
        serial: "01P00A000000000".to_owned(),
        name: "P1S".to_owned(),
        model: "P1S".to_owned(),
    }];
    for token in ["first-token", "second-token"] {
        repo.save(
            &BambuCredentials {
                user_id: "1234567890".to_owned(),
                access_token: token.to_owned(),
            },
            &devices,
        )
        .unwrap();
    }

    let account = repo.load().unwrap().unwrap();
    assert_eq!(account.credentials.access_token, "second-token");
    assert_eq!(account.devices, devices);
    assert!(!account.updated_at.is_empty());
}

#[test]
fn admin_create_and_find() {
    let pool = setup_pool();
    let repo = SqliteAdminRepository::new(pool);

    let admin = repo
        .create("admin", "$argon2id$fake_hash")
        .expect("create failed");
    assert_eq!(admin.login, "admin");

    let found = repo
        .find_by_login("admin")
        .expect("find failed")
        .expect("not found");
    assert_eq!(found.id, admin.id);
}

#[test]
fn admin_find_missing_returns_none() {
    let pool = setup_pool();
    let repo = SqliteAdminRepository::new(pool);
    assert!(repo.find_by_login("nobody").unwrap().is_none());
}

// ============================================================
// Stock-related tests
// ============================================================

/// Material 1 with orders: 200 g queued, 300 g cancelled, 400 g printed,
/// and the returned one with 150 g queued.
fn seed_stock_fixture(pool: &DbPool) -> (i64, i64, i64) {
    let user_repo = SqliteUserRepository::new(pool.clone());
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool.clone());

    let user = user_repo
        .upsert_from_cas(&CasUser {
            cas_login: "stock_user".to_owned(),
            display_name: "Stock Tester".to_owned(),
            email: "s@isep.fr".to_owned(),
            promo: None,
        })
        .unwrap();

    let material_id = seed_materials(&material_repo, &[loaded("GFA00:000000", "PLA", "Noir")])[0];

    let mut last_id = 0;
    for (weight, status) in [
        (200.0, OrderStatus::ATraiter),
        (300.0, OrderStatus::Annule),
        (400.0, OrderStatus::Imprime),
        (150.0, OrderStatus::EnTraitement),
    ] {
        let created = order_repo
            .create(NewOrder {
                user_id: user.id,
                software_used: "Cura".to_owned(),
                material_id: Some(material_id),
                quantity: 1,
                comments: None,
            })
            .unwrap();
        let mut order = order_repo.find_by_id(created.id).unwrap().unwrap();
        order.sliced_weight_grams = Some(weight);
        order.status = status;
        order_repo.update(&order).unwrap();
        last_id = created.id;
    }

    (user.id, material_id, last_id)
}

#[test]
fn reserved_weight_only_counts_orders_not_printed_yet() {
    let pool = setup_pool();
    let (_user_id, material_id, _) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let total = order_repo
        .sum_reserved_weight_by_material(material_id, None)
        .unwrap();
    assert!((total - 350.0).abs() < f64::EPSILON);
}

#[test]
fn reserved_weight_respects_exclude_order_id() {
    let pool = setup_pool();
    let (_user_id, material_id, last_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let total = order_repo
        .sum_reserved_weight_by_material(material_id, Some(last_id))
        .unwrap();
    assert!((total - 200.0).abs() < f64::EPSILON);
}

#[test]
fn reserved_weight_returns_zero_for_material_without_orders() {
    let pool = setup_pool();
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    let id = seed_materials(&material_repo, &[loaded("GFG00:FFFFFF", "PETG", "Blanc")])[0];

    let total = order_repo
        .sum_reserved_weight_by_material(id, None)
        .unwrap();
    assert_eq!(total, 0.0);
}

#[test]
fn update_if_stock_sufficient_commits_when_within_measure() {
    let pool = setup_pool();
    let (_user_id, _, last_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    // 1000 g measured, 200 g reserved by the other queued order.
    let mut order = order_repo.find_by_id(last_id).unwrap().unwrap();
    order.sliced_weight_grams = Some(800.0);
    order_repo
        .update_if_stock_sufficient(&order, 1000.0)
        .unwrap();

    let reloaded = order_repo.find_by_id(last_id).unwrap().unwrap();
    assert_eq!(reloaded.sliced_weight_grams, Some(800.0));
}

#[test]
fn update_if_stock_sufficient_rolls_back_on_overdraft() {
    let pool = setup_pool();
    let (_user_id, _, last_id) = seed_stock_fixture(&pool);
    let order_repo = SqliteOrderRepository::new(pool);

    let mut order = order_repo.find_by_id(last_id).unwrap().unwrap();
    order.sliced_weight_grams = Some(801.0);
    let err = order_repo
        .update_if_stock_sufficient(&order, 1000.0)
        .unwrap_err();

    assert!(matches!(err, DomainError::InsufficientStock { .. }));

    let reloaded = order_repo.find_by_id(last_id).unwrap().unwrap();
    assert_eq!(reloaded.sliced_weight_grams, Some(150.0));
}

#[test]
fn update_persists_material_id_change() {
    let pool = setup_pool();
    let user_repo = SqliteUserRepository::new(pool.clone());
    let material_repo = SqliteMaterialRepository::new(pool.clone());
    let order_repo = SqliteOrderRepository::new(pool);

    let user = user_repo
        .upsert_from_cas(&CasUser {
            cas_login: "swap_test".to_owned(),
            display_name: "Swap".to_owned(),
            email: "swap@isep.fr".to_owned(),
            promo: None,
        })
        .unwrap();

    let ids = seed_materials(
        &material_repo,
        &[
            loaded("GFA00:000000", "PLA", "Noir"),
            loaded("GFB00:FFFFFF", "ABS", "Blanc"),
        ],
    );

    let created = order_repo
        .create(NewOrder {
            user_id: user.id,
            software_used: "Cura".to_owned(),
            material_id: Some(ids[0]),
            quantity: 1,
            comments: None,
        })
        .unwrap();

    let mut o = order_repo.find_by_id(created.id).unwrap().unwrap();
    o.material_id = Some(ids[1]);
    order_repo.update(&o).unwrap();

    let reloaded = order_repo.find_by_id(created.id).unwrap().unwrap();
    assert_eq!(reloaded.material_id, Some(ids[1]));
}
