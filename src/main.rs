mod app;

use std::env;
use std::process;
use std::sync::Arc;

use axum::middleware;
use leptos::config::LeptosOptions;
use tower_sessions::cookie::SameSite;
use tower_sessions::SessionManagerLayer;

use fablab::application::use_cases::admin_login::AdminLoginUseCase;
use fablab::application::use_cases::get_order::GetOrderUseCase;
use fablab::application::use_cases::get_user_phone::GetUserPhoneUseCase;
use fablab::application::use_cases::list_materials::ListMaterialsUseCase;
use fablab::application::use_cases::list_orders::ListOrdersUseCase;
use fablab::application::use_cases::manage_material::ManageMaterialUseCase;
use fablab::application::use_cases::notify_order::NotifyNewOrderUseCase;
use fablab::application::use_cases::order_files::
{
    AdminDeleteOrderUseCase, CancelOrderUseCase, DeleteOrderFileUseCase,
    DownloadOrderFileUseCase, PurgeOrderFilesUseCase, StudentCancelOrderUseCase,
    UploadOrderFileUseCase,
};
use fablab::application::use_cases::submit_order::SubmitOrderUseCase;
use fablab::application::use_cases::update_order::UpdateOrderUseCase;
use fablab::application::use_cases::update_phone::UpdatePhoneUseCase;
use fablab::domain::repositories::AdminRepository;
use fablab::infrastructure::auth::argon2::{Argon2PasswordVerifier, hash_password};
use fablab::infrastructure::cas::client::CasClient;
use fablab::infrastructure::db::admin_repo::SqliteAdminRepository;
use fablab::infrastructure::db::material_repo::SqliteMaterialRepository;
use fablab::infrastructure::db::migrations::run_migrations;
use fablab::infrastructure::db::order_file_repo::SqliteOrderFileRepository;
use fablab::infrastructure::db::order_repo::SqliteOrderRepository;
use fablab::infrastructure::db::pool::DbPool;
use fablab::infrastructure::db::session_store::SqliteSessionStore;
use fablab::infrastructure::db::user_repo::SqliteUserRepository;
use fablab::infrastructure::email::config::EmailConfig;
use fablab::infrastructure::email::smtp_sender::SmtpNotificationSender;
use fablab::infrastructure::storage::local_fs::LocalFileStorage;
use fablab::infrastructure::storage::upload::UploadConfig;
use fablab::interface::admin_guard::admin_page_guard;
use fablab::interface::csrf::csrf_middleware;
use fablab::interface::routes::{AppState, AppStateParams, build_router};
use fablab::interface::security_headers::security_headers_middleware;

struct Config
{
    database_url: String,
    cas_base_url: String,
    cas_service_url: String,
    upload_dir: String,
    app_base_url: String,
}

fn load_config() -> Config
{
    let mut errors: Vec<&'static str> = Vec::new();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_|
    {
        errors.push("DATABASE_URL");
        String::new()
    });
    let cas_base_url = env::var("CAS_BASE_URL").unwrap_or_else(|_|
    {
        errors.push("CAS_BASE_URL");
        String::new()
    });
    let cas_service_url = env::var("CAS_SERVICE_URL").unwrap_or_else(|_|
    {
        errors.push("CAS_SERVICE_URL");
        String::new()
    });
    let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_|
    {
        errors.push("UPLOAD_DIR");
        String::new()
    });
    let app_base_url = env::var("APP_BASE_URL").unwrap_or_else(|_|
    {
        errors.push("APP_BASE_URL");
        String::new()
    });

    if !errors.is_empty()
    {
        eprintln!(
            "Error: missing required environment variables: {}",
            errors.join(", ")
        );
        process::exit(1);
    }

    Config { database_url, cas_base_url, cas_service_url, upload_dir, app_base_url }
}

#[tokio::main]
async fn main()
{
    if let Err(e) = dotenvy::dotenv()
    {
        eprintln!("Note: no .env file loaded ({e})");
    }

    let config = load_config();

    let email_config = match EmailConfig::from_env()
    {
        Ok(c) => c,
        Err(missing) =>
        {
            eprintln!(
                "Error: missing required email environment variables: {}",
                missing.join(", ")
            );
            process::exit(1);
        }
    };

    let upload_config = UploadConfig::from_env();

    let pool = match DbPool::open(&config.database_url)
    {
        Ok(p) => p,
        Err(e) =>
        {
            eprintln!("Error: failed to open database: {e}");
            process::exit(1);
        }
    };

    if let Err(e) = run_migrations(&pool)
    {
        eprintln!("Error: failed to run migrations: {e}");
        process::exit(1);
    }

    let storage = match LocalFileStorage::initialize(&config.upload_dir, upload_config).await
    {
        Ok(s) => Arc::new(s),
        Err(e) =>
        {
            eprintln!("Error: failed to initialize upload storage: {e}");
            process::exit(1);
        }
    };

    let order_repo = Arc::new(SqliteOrderRepository::new(pool.clone()));
    let order_file_repo = Arc::new(SqliteOrderFileRepository::new(pool.clone()));
    let user_repo = Arc::new(SqliteUserRepository::new(pool.clone()));
    let material_repo = Arc::new(SqliteMaterialRepository::new(pool.clone()));
    let admin_repo = Arc::new(SqliteAdminRepository::new(pool.clone()));

    let notifier = match SmtpNotificationSender::new(email_config)
    {
        Ok(s) => Arc::new(s),
        Err(e) =>
        {
            eprintln!("Error: failed to init SMTP sender: {e}");
            process::exit(1);
        }
    };

    seed_admin(&admin_repo);

    let session_store = match SqliteSessionStore::new(pool)
    {
        Ok(s) => s,
        Err(e) =>
        {
            eprintln!("Error: failed to create session store: {e}");
            process::exit(1);
        }
    };

    let storage_dyn: Arc<dyn fablab::domain::repositories::FileStorage> =
        Arc::clone(&storage) as Arc<dyn fablab::domain::repositories::FileStorage>;

    let submit_order = Arc::new(SubmitOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&user_repo),
        Arc::clone(&material_repo),
    ));
    let purge_order_files = Arc::new(PurgeOrderFilesUseCase::new(
        Arc::clone(&order_file_repo),
        Arc::clone(&storage_dyn),
    ));
    let update_order = Arc::new(UpdateOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&user_repo),
        Arc::clone(&material_repo),
        Arc::clone(&purge_order_files),
    ));
    let list_orders = Arc::new(ListOrdersUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&user_repo),
        Arc::clone(&material_repo),
    ));
    let get_order = Arc::new(GetOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&user_repo),
        Arc::clone(&material_repo),
        Arc::clone(&order_file_repo),
    ));
    let list_materials = Arc::new(ListMaterialsUseCase::new(Arc::clone(&material_repo)));
    let manage_material = Arc::new(ManageMaterialUseCase::new(Arc::clone(&material_repo)));
    let update_phone = Arc::new(UpdatePhoneUseCase::new(Arc::clone(&user_repo)));
    let get_user_phone = Arc::new(GetUserPhoneUseCase::new(Arc::clone(&user_repo)));
    let verifier = Arc::new(Argon2PasswordVerifier);
    let admin_login = Arc::new(AdminLoginUseCase::new(Arc::clone(&admin_repo), verifier));
    let cas_client = Arc::new(CasClient::new(config.cas_base_url));

    let upload_order_file = Arc::new(UploadOrderFileUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&order_file_repo),
        Arc::clone(&storage),
    ));
    let download_order_file = Arc::new(DownloadOrderFileUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&order_file_repo),
    ));
    let delete_order_file = Arc::new(DeleteOrderFileUseCase::new(
        Arc::clone(&order_file_repo),
        Arc::clone(&storage_dyn),
    ));
    let cancel_order = Arc::new(CancelOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&order_file_repo),
        Arc::clone(&storage_dyn),
    ));
    let student_cancel_order = Arc::new(StudentCancelOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&purge_order_files),
    ));
    let admin_delete_order = Arc::new(AdminDeleteOrderUseCase::new(
        Arc::clone(&cancel_order),
    ));
    let notify_new_order = Arc::new(NotifyNewOrderUseCase::new(
        Arc::clone(&order_repo),
        Arc::clone(&user_repo),
        Arc::clone(&material_repo),
        Arc::clone(&order_file_repo),
        Arc::clone(&notifier),
        Arc::from(config.app_base_url.as_str()),
    ));

    let leptos_options = LeptosOptions::builder()
        .output_name("fablab")
        .site_root("target/site")
        .site_addr(([0, 0, 0, 0], 3000))
        .build();

    let state = AppState::new(AppStateParams
    {
        leptos_options,
        submit_order,
        update_order,
        list_orders,
        get_order,
        list_materials,
        manage_material,
        update_phone,
        get_user_phone,
        admin_login,
        upload_order_file,
        download_order_file,
        delete_order_file,
        cancel_order,
        student_cancel_order,
        admin_delete_order,
        notify_new_order,
        storage,
        cas_client,
        cas_service_url: Arc::from(config.cas_service_url.as_str()),
        user_repo,
    });

    let is_release = !cfg!(debug_assertions);
    let session_layer = SessionManagerLayer::new(session_store)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(is_release);

    let app = build_router(state, app::App, app::shell)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn(admin_page_guard))
        .layer(middleware::from_fn(csrf_middleware))
        .layer(session_layer);

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:3000").await
    {
        Ok(l) => l,
        Err(e) =>
        {
            eprintln!("Error: failed to bind to 0.0.0.0:3000: {e}");
            process::exit(1);
        }
    };

    eprintln!("Listening on http://0.0.0.0:3000");

    if let Err(e) = axum::serve(listener, app).await
    {
        eprintln!("Error: server failed: {e}");
        process::exit(1);
    }
}

fn seed_admin(admin_repo: &Arc<SqliteAdminRepository>)
{
    let login = match env::var("ADMIN_LOGIN")
    {
        Ok(v) if !v.is_empty() => v,
        _ => return,
    };
    let password = match env::var("ADMIN_PASSWORD")
    {
        Ok(v) if !v.is_empty() => v,
        _ => return,
    };

    match admin_repo.find_by_login(&login)
    {
        Ok(Some(_)) => eprintln!("Admin account already exists: {login}"),
        Ok(None) =>
        {
            match hash_password(&password)
            {
                Ok(h) => match admin_repo.create(&login, &h)
                {
                    Ok(_) => eprintln!("Admin account created: {login}"),
                    Err(e) => eprintln!("Error: failed to create admin account: {e}"),
                },
                Err(e) => eprintln!("Error: failed to hash admin password: {e}"),
            }
        }
        Err(e) => eprintln!("Error: failed to check admin account: {e}"),
    }
}
