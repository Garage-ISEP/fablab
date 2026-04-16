#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use fablab::application::dtos::order_filter::{OrderFilter, PaymentFilter};
use fablab::application::dtos::order_output::OrderView;
use fablab::application::dtos::order_sort::{OrderSort, SortColumn, SortDirection};
use fablab::domain::errors::DomainError;
use fablab::domain::material::Material;
use fablab::domain::order::{NewOrder, Order, OrderStatus};
use fablab::domain::repositories::
{
    AdminRepository, MaterialRepository, OrderRepository,
    PasswordVerifier, UserRepository,
};
use fablab::domain::user::{AdminUser, CasUser, User};

use fablab::application::dtos::caller::Caller;
use fablab::application::dtos::order_input::{SubmitOrderInput, UpdateOrderInput};
use fablab::application::errors::AppError;
use fablab::application::use_cases::admin_login::AdminLoginUseCase;
use fablab::application::use_cases::get_order::GetOrderUseCase;
use fablab::application::use_cases::list_orders::ListOrdersUseCase;
use fablab::application::use_cases::order_files::PurgeOrderFilesUseCase;
use fablab::application::use_cases::submit_order::SubmitOrderUseCase;
use fablab::application::use_cases::update_order::UpdateOrderUseCase;

// ============================================================================
// Mock repositories
// ============================================================================

#[derive(Default)]
struct MockOrderRepo
{
    orders: Mutex<Vec<Order>>,
    next_id: Mutex<i64>,
}

impl OrderRepository for MockOrderRepo
{
    fn find_all(&self) -> Result<Vec<Order>, DomainError>
    {
        Ok(self.orders.lock().unwrap().clone())
    }

    fn find_by_user(&self, user_id: i64) -> Result<Vec<Order>, DomainError>
    {
        let orders = self.orders.lock().unwrap();
        Ok(orders.iter().filter(|o| o.user_id == user_id).cloned().collect())
    }

    fn find_by_id(&self, id: i64) -> Result<Option<Order>, DomainError>
    {
        let orders = self.orders.lock().unwrap();
        Ok(orders.iter().find(|o| o.id == id).cloned())
    }

    fn create(&self, new: NewOrder) -> Result<Order, DomainError>
    {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        let id = *next;

        let order = Order
        {
            id,
            user_id: new.user_id,
            created_at: "2026-04-13T00:00:00".to_owned(),
            software_used: new.software_used,
            material_id: new.material_id,
            quantity: new.quantity,
            comments: new.comments,
            status: OrderStatus::ATraiter,
            requires_payment: false,
            sliced_weight_grams: None,
            print_time_minutes: None,
        };

        self.orders.lock().unwrap().push(order.clone());
        Ok(order)
    }

    fn update(&self, order: &Order) -> Result<(), DomainError>
    {
        let mut orders = self.orders.lock().unwrap();
        if let Some(existing) = orders.iter_mut().find(|o| o.id == order.id)
        {
            *existing = order.clone();
            Ok(())
        }
        else
        {
            Err(DomainError::OrderNotFound { id: order.id })
        }
    }

    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        let mut orders = self.orders.lock().unwrap();
        let before = orders.len();
        orders.retain(|o| o.id != id);
        if orders.len() == before
        {
            Err(DomainError::OrderNotFound { id })
        }
        else
        {
            Ok(())
        }
    }
}

struct MockUserRepo
{
    users: Mutex<Vec<User>>,
    phone_updates: Mutex<Vec<(i64, String)>>,
}

impl MockUserRepo
{
    fn new(users: Vec<User>) -> Self
    {
        Self
        {
            users: Mutex::new(users),
            phone_updates: Mutex::new(Vec::new()),
        }
    }

    fn phone_updates(&self) -> Vec<(i64, String)>
    {
        self.phone_updates.lock().unwrap().clone()
    }
}

impl UserRepository for MockUserRepo
{
    fn find_by_id(&self, id: i64) -> Result<Option<User>, DomainError>
    {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.id == id).cloned())
    }

    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<User>, DomainError>
    {
        let users = self.users.lock().unwrap();
        Ok(users.iter().filter(|u| ids.contains(&u.id)).cloned().collect())
    }

    fn find_by_cas_login(&self, login: &str) -> Result<Option<User>, DomainError>
    {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.cas_login == login).cloned())
    }

    fn upsert_from_cas(&self, cas_user: &CasUser) -> Result<User, DomainError>
    {
        let mut users = self.users.lock().unwrap();
        if let Some(existing) = users.iter_mut().find(|u| u.cas_login == cas_user.cas_login)
        {
            existing.display_name = cas_user.display_name.clone();
            existing.email = cas_user.email.clone();
            existing.promo = cas_user.promo.clone();
            Ok(existing.clone())
        }
        else
        {
            let user = User
            {
                id: users.len() as i64 + 1,
                cas_login: cas_user.cas_login.clone(),
                display_name: cas_user.display_name.clone(),
                email: cas_user.email.clone(),
                phone: None,
                promo: cas_user.promo.clone(),
                created_at: "2026-04-13T00:00:00".to_owned(),
            };
            users.push(user.clone());
            Ok(user)
        }
    }

    fn update_phone(&self, user_id: i64, phone: &str) -> Result<(), DomainError>
    {
        let users = self.users.lock().unwrap();
        if users.iter().any(|u| u.id == user_id)
        {
            self.phone_updates.lock().unwrap().push((user_id, phone.to_owned()));
            Ok(())
        }
        else
        {
            Err(DomainError::UserNotFound { id: user_id })
        }
    }
}

#[derive(Default)]
struct MockMaterialRepo
{
    materials: Mutex<Vec<Material>>,
}

impl MaterialRepository for MockMaterialRepo
{
    fn find_all(&self) -> Result<Vec<Material>, DomainError>
    {
        Ok(self.materials.lock().unwrap().clone())
    }

    fn find_available(&self) -> Result<Vec<Material>, DomainError>
    {
        let mats = self.materials.lock().unwrap();
        Ok(mats.iter().filter(|m| m.available).cloned().collect())
    }

    fn find_by_id(&self, id: i64) -> Result<Option<Material>, DomainError>
    {
        let mats = self.materials.lock().unwrap();
        Ok(mats.iter().find(|m| m.id == id).cloned())
    }

    fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<Material>, DomainError>
    {
        let mats = self.materials.lock().unwrap();
        Ok(mats.iter().filter(|m| ids.contains(&m.id)).cloned().collect())
    }

    fn upsert(&self, material: &Material) -> Result<(), DomainError>
    {
        let mut mats = self.materials.lock().unwrap();
        if let Some(existing) = mats.iter_mut().find(|m| m.id == material.id)
        {
            *existing = material.clone();
        }
        else
        {
            mats.push(material.clone());
        }
        Ok(())
    }

    fn max_id(&self) -> Result<i64, DomainError>
    {
        let mats = self.materials.lock().unwrap();
        Ok(mats.iter().map(|m| m.id).max().unwrap_or(0))
    }

    fn count_orders_using(&self, _id: i64) -> Result<i64, DomainError>
    {
        Ok(0)
    }

    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        let mut mats = self.materials.lock().unwrap();
        let len_before = mats.len();
        mats.retain(|m| m.id != id);
        if mats.len() == len_before
        {
            return Err(DomainError::MaterialNotFound { id });
        }
        Ok(())
    }
}

struct MockAdminRepo
{
    admins: Mutex<Vec<AdminUser>>,
}

impl MockAdminRepo
{
    fn new(admins: Vec<AdminUser>) -> Self
    {
        Self { admins: Mutex::new(admins) }
    }
}

impl AdminRepository for MockAdminRepo
{
    fn find_by_login(&self, login: &str) -> Result<Option<AdminUser>, DomainError>
    {
        let admins = self.admins.lock().unwrap();
        Ok(admins.iter().find(|a| a.login == login).cloned())
    }

    fn create(&self, login: &str, password_hash: &str) -> Result<AdminUser, DomainError>
    {
        let mut admins = self.admins.lock().unwrap();
        let id = admins.len() as i64 + 1;
        let admin = AdminUser
        {
            id,
            login: login.to_owned(),
            password_hash: password_hash.to_owned(),
        };
        admins.push(admin.clone());
        Ok(admin)
    }
}

struct MockPasswordVerifier;

impl PasswordVerifier for MockPasswordVerifier
{
    fn verify(&self, plain: &str, _hash: &str) -> Result<bool, DomainError>
    {
        Ok(plain == "correct_password")
    }
}

// ============================================================================
// Helpers
// ============================================================================

use fablab::domain::order_file::{NewOrderFile, OrderFile, StorageStats};

#[derive(Default)]
struct MockFileStorage
{
    deleted: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl fablab::domain::repositories::FileStorage for MockFileStorage
{
    async fn delete(&self, stored_filename: &str) -> Result<(), DomainError>
    {
        self.deleted.lock().unwrap().push(stored_filename.to_owned());
        Ok(())
    }
}

#[derive(Default)]
struct MockOrderFileRepo
{
    files: Mutex<Vec<OrderFile>>,
    next_id: Mutex<i64>,
}

impl fablab::domain::repositories::OrderFileRepository for MockOrderFileRepo
{
    fn create(&self, f: NewOrderFile) -> Result<OrderFile, DomainError>
    {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        let id = *next;
        let row = OrderFile
        {
            id,
            order_id: f.order_id,
            original_filename: f.original_filename,
            stored_filename: f.stored_filename,
            size_bytes: f.size_bytes,
            mime_type: f.mime_type,
            uploaded_at: "2026-04-13T00:00:00".to_owned(),
        };
        self.files.lock().unwrap().push(row.clone());
        Ok(row)
    }
    fn find_by_id(&self, id: i64) -> Result<Option<OrderFile>, DomainError>
    {
        Ok(self.files.lock().unwrap().iter().find(|f| f.id == id).cloned())
    }
    fn find_by_order(&self, order_id: i64) -> Result<Vec<OrderFile>, DomainError>
    {
        Ok(self.files.lock().unwrap().iter().filter(|f| f.order_id == order_id).cloned().collect())
    }
    fn count_by_order(&self, order_id: i64) -> Result<i64, DomainError>
    {
        Ok(self.files.lock().unwrap().iter().filter(|f| f.order_id == order_id).count() as i64)
    }
    fn delete(&self, id: i64) -> Result<(), DomainError>
    {
        let mut files = self.files.lock().unwrap();
        let before = files.len();
        files.retain(|f| f.id != id);
        if files.len() == before
        {
            Err(DomainError::Database(format!("file {id} not found")))
        }
        else
        {
            Ok(())
        }
    }
    fn storage_stats(&self) -> Result<StorageStats, DomainError>
    {
        let files = self.files.lock().unwrap();
        Ok(StorageStats
        {
            total_files: files.len() as i64,
            total_bytes: files.iter().map(|f| f.size_bytes).sum(),
        })
    }
}

fn test_user() -> User
{
    User
    {
        id: 1,
        cas_login: "test001".to_owned(),
        display_name: "Test User".to_owned(),
        email: "test@isep.fr".to_owned(),
        phone: None,
        promo: Some("A2".to_owned()),
        created_at: "2026-04-13T00:00:00".to_owned(),
    }
}

fn test_user_2() -> User
{
    User
    {
        id: 2,
        cas_login: "other002".to_owned(),
        display_name: "Other User".to_owned(),
        email: "other@isep.fr".to_owned(),
        phone: None,
        promo: None,
        created_at: "2026-04-13T00:00:00".to_owned(),
    }
}

fn seed_order(repo: &MockOrderRepo, user_id: i64) -> Order
{
    repo.create(NewOrder
    {
        user_id,
        software_used: "Cura".to_owned(),
        material_id: None,
        quantity: 1,
        comments: None,
    })
    .unwrap()
}

// ============================================================================
// submit_order tests
// ============================================================================

#[tokio::test]
async fn submit_order_creates_order_and_updates_phone()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let uc = SubmitOrderUseCase::new(
        Arc::clone(&orders),
        Arc::clone(&users),
        Arc::clone(&materials),
    );

    let input = SubmitOrderInput
    {
        user_id: 1,
        software_used: "PrusaSlicer".to_owned(),
        material_id: None,
        quantity: 3,
        comments: None,
        phone: Some("0612345678".to_owned()),
    };

    let order_id = uc.execute(input).expect("submit_order failed");
    assert!(order_id > 0);

    let stored = orders.find_by_id(order_id).unwrap().unwrap();
    assert_eq!(stored.user_id, 1);
    assert_eq!(stored.quantity, 3);
    assert_eq!(stored.status, OrderStatus::ATraiter);
    assert!(!stored.requires_payment);

    let updates = users.phone_updates();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0], (1, "0612345678".to_owned()));
}

#[tokio::test]
async fn submit_order_validates_quantity()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let uc = SubmitOrderUseCase::new(orders, users, materials);

    let input = SubmitOrderInput
    {
        user_id: 1,
        software_used: "Cura".to_owned(),
        material_id: None,
        quantity: 0,
        comments: None,
        phone: None,
    };

    let result = uc.execute(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
}

#[tokio::test]
async fn submit_order_validates_phone_format()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let uc = SubmitOrderUseCase::new(orders, users, materials);

    let input = SubmitOrderInput
    {
        user_id: 1,
        software_used: "Cura".to_owned(),
        material_id: None,
        quantity: 1,
        comments: None,
        phone: Some("abc".to_owned()),
    };

    let result = uc.execute(input);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
}

// ============================================================================
// get_order tests
// ============================================================================

#[test]
fn get_order_student_can_see_own_order()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let order = seed_order(&orders, 1);

    let files = Arc::new(MockOrderFileRepo::default());
    let uc = GetOrderUseCase::new(orders, users, materials, files);

    let caller = Caller::Student { user_id: 1 };
    let view = uc.execute(order.id, &caller).expect("get_order failed");
    assert_eq!(view.id, order.id);
}

#[test]
fn get_order_student_cannot_see_other_users_order()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user(), test_user_2()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let order = seed_order(&orders, 1);

    let files = Arc::new(MockOrderFileRepo::default());
    let uc = GetOrderUseCase::new(orders, users, materials, files);

    let caller = Caller::Student { user_id: 2 };
    let result = uc.execute(order.id, &caller);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotAuthorized));
}

#[test]
fn get_order_admin_can_see_any_order()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let order = seed_order(&orders, 1);

    let files = Arc::new(MockOrderFileRepo::default());
    let uc = GetOrderUseCase::new(orders, users, materials, files);

    let caller = Caller::Admin;
    let view = uc.execute(order.id, &caller).expect("admin get_order failed");
    assert_eq!(view.id, order.id);
}

// ============================================================================
// list_orders tests
// ============================================================================

#[test]
fn list_orders_admin_sees_all()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user(), test_user_2()]));
    let materials = Arc::new(MockMaterialRepo::default());

    seed_order(&orders, 1);
    seed_order(&orders, 2);
    seed_order(&orders, 1);

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let views = uc.execute(&Caller::Admin).expect("list_orders admin failed");
    assert_eq!(views.len(), 3);
}

#[test]
fn list_orders_student_sees_only_own()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user(), test_user_2()]));
    let materials = Arc::new(MockMaterialRepo::default());

    seed_order(&orders, 1);
    seed_order(&orders, 2);
    seed_order(&orders, 1);

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let caller = Caller::Student { user_id: 1 };
    let views = uc.execute(&caller).expect("list_orders student failed");
    assert_eq!(views.len(), 2);
    assert!(views.iter().all(|v| v.user_id == 1));
}

// ============================================================================
// update_order tests
// ============================================================================

#[tokio::test]
async fn update_order_admin_can_update()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());
    let order_files = Arc::new(MockOrderFileRepo::default());
    let storage: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::new(MockFileStorage::default());
    let purge = Arc::new(PurgeOrderFilesUseCase::new(order_files, storage));

    let order = seed_order(&orders, 1);

    let uc = UpdateOrderUseCase::new(orders, users, materials, purge);

    let input = UpdateOrderInput
    {
        order_id: order.id,
        status: Some("en_traitement".to_owned()),
        requires_payment: Some(true),
        sliced_weight_grams: Some(25.0),
        print_time_minutes: Some(120),
    };

    let view = uc.execute(input, &Caller::Admin).await.expect("update_order admin failed");
    assert_eq!(view.status, "en_traitement");
    assert!(view.requires_payment);
    assert_eq!(view.sliced_weight_grams, Some(25.0));
}

#[tokio::test]
async fn update_order_student_is_rejected()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());
    let order_files = Arc::new(MockOrderFileRepo::default());
    let storage: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::new(MockFileStorage::default());
    let purge = Arc::new(PurgeOrderFilesUseCase::new(order_files, storage));

    let order = seed_order(&orders, 1);

    let uc = UpdateOrderUseCase::new(orders, users, materials, purge);

    let input = UpdateOrderInput
    {
        order_id: order.id,
        status: None,
        requires_payment: Some(true),
        sliced_weight_grams: None,
        print_time_minutes: None,
    };

    let caller = Caller::Student { user_id: 1 };
    let result = uc.execute(input, &caller).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotAuthorized));
}

#[tokio::test]
async fn update_order_invalid_status_transition_rejected()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());
    let order_files = Arc::new(MockOrderFileRepo::default());
    let storage: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::new(MockFileStorage::default());
    let purge = Arc::new(PurgeOrderFilesUseCase::new(order_files, storage));

    let _order = seed_order(&orders, 1);

    let uc = UpdateOrderUseCase::new(orders, users, materials, purge);

    let input = UpdateOrderInput
    {
        order_id: 1,
        status: Some("livre".to_owned()),
        requires_payment: None,
        sliced_weight_grams: None,
        print_time_minutes: None,
    };

    let result = uc.execute(input, &Caller::Admin).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
}

#[tokio::test]
async fn update_order_rejects_negative_weight()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());
    let order_files = Arc::new(MockOrderFileRepo::default());
    let storage: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::new(MockFileStorage::default());
    let purge = Arc::new(PurgeOrderFilesUseCase::new(order_files, storage));

    let _order = seed_order(&orders, 1);

    let uc = UpdateOrderUseCase::new(orders, users, materials, purge);

    let input = UpdateOrderInput
    {
        order_id: 1,
        status: None,
        requires_payment: None,
        sliced_weight_grams: Some(-5.0),
        print_time_minutes: None,
    };

    let result = uc.execute(input, &Caller::Admin).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
}

#[tokio::test]
async fn update_order_rejects_negative_print_time()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());
    let order_files = Arc::new(MockOrderFileRepo::default());
    let storage: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::new(MockFileStorage::default());
    let purge = Arc::new(PurgeOrderFilesUseCase::new(order_files, storage));

    let _order = seed_order(&orders, 1);

    let uc = UpdateOrderUseCase::new(orders, users, materials, purge);

    let input = UpdateOrderInput
    {
        order_id: 1,
        status: None,
        requires_payment: None,
        sliced_weight_grams: None,
        print_time_minutes: Some(-10),
    };

    let result = uc.execute(input, &Caller::Admin).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::InvalidInput(_)));
}

// ============================================================================
// admin_login tests
// ============================================================================

#[test]
fn admin_login_success()
{
    let admins = Arc::new(MockAdminRepo::new(vec![AdminUser
    {
        id: 1,
        login: "admin".to_owned(),
        password_hash: "hashed".to_owned(),
    }]));
    let verifier = Arc::new(MockPasswordVerifier);
    let uc = AdminLoginUseCase::new(admins, verifier);

    let result = uc.execute("admin", "correct_password");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().login, "admin");
}

#[test]
fn admin_login_wrong_password()
{
    let admins = Arc::new(MockAdminRepo::new(vec![AdminUser
    {
        id: 1,
        login: "admin".to_owned(),
        password_hash: "hashed".to_owned(),
    }]));
    let verifier = Arc::new(MockPasswordVerifier);
    let uc = AdminLoginUseCase::new(admins, verifier);

    let result = uc.execute("admin", "wrong_password");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotAuthorized));
}

#[test]
fn admin_login_unknown_user()
{
    let admins = Arc::new(MockAdminRepo::new(vec![]));
    let verifier = Arc::new(MockPasswordVerifier);
    let uc = AdminLoginUseCase::new(admins, verifier);

    let result = uc.execute("nobody", "correct_password");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::NotAuthorized));
}

#[test]
fn order_sort_by_id_asc()
{
    let mut orders = [make_view(3), make_view(1), make_view(2)];
    let sort = OrderSort::new(SortColumn::Id, SortDirection::Asc);
    orders.sort_by(|a, b| sort.compare(a, b));
    assert_eq!(orders.iter().map(|o| o.id).collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn order_sort_by_id_desc()
{
    let mut orders = [make_view(1), make_view(3), make_view(2)];
    let sort = OrderSort::new(SortColumn::Id, SortDirection::Desc);
    orders.sort_by(|a, b| sort.compare(a, b));
    assert_eq!(orders.iter().map(|o| o.id).collect::<Vec<_>>(), vec![3, 2, 1]);
}

#[test]
fn order_sort_by_payment_groups_gratuit_first_when_asc()
{
    let mut a = make_view(1); a.requires_payment = true;
    let mut b = make_view(2); b.requires_payment = false;
    let mut c = make_view(3); c.requires_payment = true;

    let mut orders = [a, b, c];
    let sort = OrderSort::new(SortColumn::RequiresPayment, SortDirection::Asc);
    orders.sort_by(|a, b| sort.compare(a, b));
    // false < true, so the gratuit (false) comes first
    assert!(!orders[0].requires_payment);
    assert!(orders[1].requires_payment);
    assert!(orders[2].requires_payment);
}

#[test]
fn list_orders_filters_by_status()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let _ = seed_order(&orders, 1);
    let _ = seed_order(&orders, 1);

    // Move one to "annule"
    let mut all = orders.find_all().unwrap();
    all[0].status = OrderStatus::Annule;
    orders.update(&all[0]).unwrap();

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let filter = OrderFilter
    {
        status: Some("annule".to_owned()),
        payment: None,
        search: None,
    };
    let result = uc.execute_filtered(&Caller::Admin, &filter, OrderSort::default_recent())
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].status, "annule");
}

#[test]
fn list_orders_filters_by_payment_required()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    let _ = seed_order(&orders, 1);
    let _ = seed_order(&orders, 1);

    let mut all = orders.find_all().unwrap();
    all[0].requires_payment = true;
    orders.update(&all[0]).unwrap();

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let filter = OrderFilter
    {
        status: None,
        payment: Some(PaymentFilter::Requires),
        search: None,
    };
    let result = uc.execute_filtered(&Caller::Admin, &filter, OrderSort::default_recent())
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].requires_payment);
}

#[test]
fn list_orders_search_by_id()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    seed_order(&orders, 1);
    seed_order(&orders, 1);
    seed_order(&orders, 1);

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let filter = OrderFilter
    {
        status: None,
        payment: None,
        search: Some("2".to_owned()),
    };
    let result = uc.execute_filtered(&Caller::Admin, &filter, OrderSort::default_recent())
        .unwrap();
    // matches order id "2"
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 2);
}

#[test]
fn list_orders_sort_by_id_asc()
{
    let orders = Arc::new(MockOrderRepo::default());
    let users = Arc::new(MockUserRepo::new(vec![test_user()]));
    let materials = Arc::new(MockMaterialRepo::default());

    seed_order(&orders, 1);
    seed_order(&orders, 1);
    seed_order(&orders, 1);

    let uc = ListOrdersUseCase::new(orders, users, materials);

    let sort = OrderSort::new(SortColumn::Id, SortDirection::Asc);
    let result = uc.execute_filtered(&Caller::Admin, &OrderFilter::default(), sort)
        .unwrap();
    let ids: Vec<i64> = result.iter().map(|o| o.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

fn make_view(id: i64) -> OrderView
{
    OrderView
    {
        id,
        user_id: 1,
        user_display_name: format!("User {id}"),
        created_at: "2026-04-13T00:00:00".to_owned(),
        files: Vec::new(),
        software_used: String::new(),
        material_label: None,
        quantity: 1,
        comments: None,
        status: "a_traiter".to_owned(),
        requires_payment: false,
        sliced_weight_grams: None,
        print_time_minutes: None,
    }
}