use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Form, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use leptos::config::LeptosOptions;
use serde::Deserialize;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::Session;

use crate::application::use_cases::admin_login::AdminLoginUseCase;
use crate::application::use_cases::get_order::GetOrderUseCase;
use crate::application::use_cases::get_user_phone::GetUserPhoneUseCase;
use crate::application::use_cases::list_materials::ListMaterialsUseCase;
use crate::application::use_cases::list_orders::ListOrdersUseCase;
use crate::application::use_cases::manage_material::ManageMaterialUseCase;
use crate::application::use_cases::notify_order::NotifyNewOrderUseCase;
use crate::application::use_cases::order_files::
{
    AdminDeleteOrderUseCase, CancelOrderUseCase, DeleteOrderFileUseCase, DownloadOrderFileUseCase,
    StudentCancelOrderUseCase, UploadOrderFileUseCase,
};
use crate::application::use_cases::submit_order::SubmitOrderUseCase;
use crate::application::use_cases::update_order::UpdateOrderUseCase;
use crate::application::use_cases::update_phone::UpdatePhoneUseCase;
use crate::domain::repositories::UserRepository;
use crate::infrastructure::auth::argon2::Argon2PasswordVerifier;
use crate::infrastructure::cas::client::CasClient;
use crate::infrastructure::db::admin_repo::SqliteAdminRepository;
use crate::infrastructure::db::material_repo::SqliteMaterialRepository;
use crate::infrastructure::db::order_file_repo::SqliteOrderFileRepository;
use crate::infrastructure::db::order_repo::SqliteOrderRepository;
use crate::infrastructure::db::user_repo::SqliteUserRepository;
use crate::infrastructure::email::smtp_sender::SmtpNotificationSender;
use crate::infrastructure::storage::local_fs::LocalFileStorage;
use crate::interface::file_handlers::
{
    admin_delete_material_handler, admin_delete_order_handler, delete_file_handler,
    download_file_handler, student_cancel_order_handler, upload_order_handler,
};
use crate::interface::flash::{FlashLevel, set_flash};

type ConcreteSubmitOrder =
    SubmitOrderUseCase<SqliteOrderRepository, SqliteUserRepository, SqliteMaterialRepository>;
type ConcreteUpdateOrder = UpdateOrderUseCase<
    SqliteOrderRepository,
    SqliteUserRepository,
    SqliteMaterialRepository,
    SqliteOrderFileRepository,
>;
type ConcreteListOrders =
    ListOrdersUseCase<SqliteOrderRepository, SqliteUserRepository, SqliteMaterialRepository>;
type ConcreteGetOrder = GetOrderUseCase<
    SqliteOrderRepository,
    SqliteUserRepository,
    SqliteMaterialRepository,
    SqliteOrderFileRepository,
>;
type ConcreteListMaterials = ListMaterialsUseCase<SqliteMaterialRepository>;
type ConcreteManageMaterial = ManageMaterialUseCase<SqliteMaterialRepository>;
type ConcreteUpdatePhone = UpdatePhoneUseCase<SqliteUserRepository>;
type ConcreteGetUserPhone = GetUserPhoneUseCase<SqliteUserRepository>;
type ConcreteAdminLogin = AdminLoginUseCase<SqliteAdminRepository, Argon2PasswordVerifier>;
type ConcreteUploadFile = UploadOrderFileUseCase<SqliteOrderRepository, SqliteOrderFileRepository>;
type ConcreteDownloadFile =
    DownloadOrderFileUseCase<SqliteOrderRepository, SqliteOrderFileRepository>;
type ConcreteDeleteFile = DeleteOrderFileUseCase<SqliteOrderFileRepository>;
type ConcreteCancelOrder = CancelOrderUseCase<SqliteOrderRepository, SqliteOrderFileRepository>;
type ConcreteStudentCancel = StudentCancelOrderUseCase<
    SqliteOrderRepository,
    SqliteUserRepository,
    SqliteMaterialRepository,
    SqliteOrderFileRepository,
>;
type ConcreteAdminDelete =
    AdminDeleteOrderUseCase<SqliteOrderRepository, SqliteOrderFileRepository>;
type ConcreteNotifyOrder = NotifyNewOrderUseCase<
    SqliteOrderRepository,
    SqliteUserRepository,
    SqliteMaterialRepository,
    SqliteOrderFileRepository,
    SmtpNotificationSender,
>;

#[derive(Clone)]
pub struct AppState
{
    pub leptos_options: LeptosOptions,
    pub submit_order: Arc<ConcreteSubmitOrder>,
    pub update_order: Arc<ConcreteUpdateOrder>,
    pub list_orders: Arc<ConcreteListOrders>,
    pub get_order: Arc<ConcreteGetOrder>,
    pub list_materials: Arc<ConcreteListMaterials>,
    pub manage_material: Arc<ConcreteManageMaterial>,
    pub update_phone: Arc<ConcreteUpdatePhone>,
    pub get_user_phone: Arc<ConcreteGetUserPhone>,
    pub admin_login: Arc<ConcreteAdminLogin>,
    pub upload_order_file: Arc<ConcreteUploadFile>,
    pub download_order_file: Arc<ConcreteDownloadFile>,
    pub delete_order_file: Arc<ConcreteDeleteFile>,
    pub cancel_order: Arc<ConcreteCancelOrder>,
    pub student_cancel_order: Arc<ConcreteStudentCancel>,
    pub admin_delete_order: Arc<ConcreteAdminDelete>,
    pub notify_new_order: Arc<ConcreteNotifyOrder>,
    pub storage: Arc<LocalFileStorage>,
    pub cas_client: Arc<CasClient>,
    pub cas_service_url: Arc<str>,
    user_repo: Arc<SqliteUserRepository>,
}

pub struct AppStateParams
{
    pub leptos_options: LeptosOptions,
    pub submit_order: Arc<ConcreteSubmitOrder>,
    pub update_order: Arc<ConcreteUpdateOrder>,
    pub list_orders: Arc<ConcreteListOrders>,
    pub get_order: Arc<ConcreteGetOrder>,
    pub list_materials: Arc<ConcreteListMaterials>,
    pub manage_material: Arc<ConcreteManageMaterial>,
    pub update_phone: Arc<ConcreteUpdatePhone>,
    pub get_user_phone: Arc<ConcreteGetUserPhone>,
    pub admin_login: Arc<ConcreteAdminLogin>,
    pub upload_order_file: Arc<ConcreteUploadFile>,
    pub download_order_file: Arc<ConcreteDownloadFile>,
    pub delete_order_file: Arc<ConcreteDeleteFile>,
    pub cancel_order: Arc<ConcreteCancelOrder>,
    pub student_cancel_order: Arc<ConcreteStudentCancel>,
    pub admin_delete_order: Arc<ConcreteAdminDelete>,
    pub notify_new_order: Arc<ConcreteNotifyOrder>,
    pub storage: Arc<LocalFileStorage>,
    pub cas_client: Arc<CasClient>,
    pub cas_service_url: Arc<str>,
    pub user_repo: Arc<SqliteUserRepository>,
}

impl AppState
{
    pub fn new(p: AppStateParams) -> Self
    {
        Self 
        {
            leptos_options: p.leptos_options,
            submit_order: p.submit_order,
            update_order: p.update_order,
            list_orders: p.list_orders,
            get_order: p.get_order,
            list_materials: p.list_materials,
            manage_material: p.manage_material,
            update_phone: p.update_phone,
            get_user_phone: p.get_user_phone,
            admin_login: p.admin_login,
            upload_order_file: p.upload_order_file,
            download_order_file: p.download_order_file,
            delete_order_file: p.delete_order_file,
            cancel_order: p.cancel_order,
            student_cancel_order: p.student_cancel_order,
            admin_delete_order: p.admin_delete_order,
            notify_new_order: p.notify_new_order,
            storage: p.storage,
            cas_client: p.cas_client,
            cas_service_url: p.cas_service_url,
            user_repo: p.user_repo,
        }
    }
}

impl axum::extract::FromRef<AppState> for LeptosOptions
{
    fn from_ref(state: &AppState) -> Self
    {
        state.leptos_options.clone()
    }
}

pub fn build_router<AppFn, AppIV, ShellFn, ShellIV>
(
    state: AppState,
    app_fn: AppFn,
    shell_fn: ShellFn,
) -> axum::Router
where
    AppFn: Fn() -> AppIV + Clone + Send + Sync + 'static,
    AppIV: leptos::prelude::IntoView + 'static,
    ShellFn: Fn() -> ShellIV + Clone + Send + Sync + 'static,
    ShellIV: leptos::prelude::IntoView + 'static,
{
    use leptos_axum::{LeptosRoutes, generate_route_list};

    let paths = generate_route_list(app_fn);

    let state_for_context = state.clone();
    let shell_with_context = move || {
        leptos::prelude::provide_context(state_for_context.clone());
        shell_fn()
    };

    let cfg = state.storage.config();
    let files_cap: u64 = u64::try_from(cfg.max_files_per_order.clamp(1, 100)).unwrap_or(1);
    let upload_limit = cfg
        .max_upload_bytes
        .saturating_mul(files_cap)
        .saturating_add(64 * 1024);
    let upload_limit = usize::try_from(upload_limit).unwrap_or(usize::MAX);

    axum::Router::new()
        .route("/auth/cas", get(cas_redirect_handler))
        .route("/auth/cas/callback", get(cas_callback_handler))
        .route("/auth/admin/login", post(admin_login_handler))
        .route("/auth/logout", post(logout_handler))
        .route
        (
            "/orders",
            post(upload_order_handler).layer(DefaultBodyLimit::max(upload_limit)),
        )
        .route("/admin/files/{id}/download", get(download_file_handler))
        .route("/admin/files/{id}/delete", post(delete_file_handler))
        .route("/my-orders/{id}/cancel", post(student_cancel_order_handler))
        .route("/admin/order/{id}/delete", post(admin_delete_order_handler))
        .route("/admin/materials/{id}/delete", post(admin_delete_material_handler))
        .nest_service("/style", ServeDir::new("style"))
        .nest_service("/public", ServeDir::new("public")) 
        .route_service("/favicon.ico", ServeFile::new("public/favicon.ico"))
        .leptos_routes(&state, paths, shell_with_context)
        .with_state(state)
}

async fn cas_redirect_handler(State(state): State<AppState>) -> Redirect
{
    let service = &state.cas_service_url;
    let url = format!("{}/login?service={}", state.cas_client.base_url(), service);
    Redirect::temporary(&url)
}

#[derive(Deserialize)]
pub struct CasCallbackParams
{
    ticket: String,
}

async fn cas_callback_handler
(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<CasCallbackParams>,
) -> Response
{
    let cas_client = state.cas_client.clone();
    let service_url = state.cas_service_url.clone();
    let user_repo = state.user_repo.clone();
    let ticket = params.ticket;

    let result = tokio::task::spawn_blocking(move || {
        let cas_user = cas_client.validate_ticket(&ticket, &service_url)?;
        user_repo.upsert_from_cas(&cas_user)
    })
    .await;

    let user = match result
    {
        Ok(Ok(user)) => user,
        _ =>
        {
            let _ = set_flash
            (
                &session,
                FlashLevel::Error,
                "Echec de l'authentification CAS. Veuillez reessayer.",
            )
            .await;
            return Redirect::to("/").into_response();
        }
    };

    if let Err(e) = cycle_session(&session, user.id, "student").await
    {
        eprintln!("Session cycle error: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Session error").into_response();
    }

    let _ = set_flash(&session, FlashLevel::Success, "Connexion reussie.").await;
    Redirect::to("/my-orders").into_response()
}

#[derive(Deserialize)]
struct AdminLoginForm
{
    login: String,
    password: String,
    #[serde(rename = "_csrf")]
    _csrf: String,
}

async fn admin_login_handler(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<AdminLoginForm>,
) -> Response
{
    let admin_login = state.admin_login.clone();
    let login = form.login;
    let password = form.password;
    let result = tokio::task::spawn_blocking(move ||
    {
        admin_login.execute(&login, &password)
    })
    .await;

    let admin = match result
    {
        Ok(Ok(a)) => a,
        _ =>
        {
            let _ = set_flash(
                &session,
                FlashLevel::Error,
                "Identifiant ou mot de passe incorrect.",
            )
            .await;
            return Redirect::to("/admin/login").into_response();
        }
    };

    if let Err(e) = cycle_session(&session, admin.id, "admin").await
    {
        eprintln!("Session cycle error: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Session error").into_response();
    }

    let _ = set_flash(
        &session,
        FlashLevel::Success,
        "Connexion administrateur reussie.",
    )
    .await;
    Redirect::to("/admin").into_response()
}

async fn logout_handler(session: Session) -> Response
{
    session.clear().await;
    if session.delete().await.is_err()
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Session error").into_response();
    }
    Redirect::to("/").into_response()
}

async fn cycle_session(session: &Session, user_id: i64, role: &str) -> Result<(), String>
{
    session.clear().await;
    session
        .cycle_id()
        .await
        .map_err(|e| format!("cycle_id failed: {e}"))?;
    session
        .insert("user_id", user_id)
        .await
        .map_err(|e| format!("insert user_id failed: {e}"))?;
    session
        .insert("role", role)
        .await
        .map_err(|e| format!("insert role failed: {e}"))?;
    Ok(())
}
