use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use futures_util::StreamExt;
use tokio_util::io::ReaderStream;
use tower_sessions::Session;

use crate::application::dtos::caller::Caller;
use crate::application::dtos::order_input::SubmitOrderInput;
use crate::application::errors::AppError;
use crate::infrastructure::storage::upload::sanitize_download_name;
use crate::interface::csrf::validate_csrf_token;
use crate::interface::error_messages::user_message;
use crate::interface::flash::{FlashLevel, set_flash};
use crate::interface::routes::AppState;

/// Fields captured from the multipart body while streaming. File parts
/// are handled inline (streamed to storage) and never buffered here.
#[derive(Default)]
struct OrderFormFields
{
    csrf: Option<String>,
    software_used: Option<String>,
    material_id: Option<String>,
    quantity: Option<String>,
    comments: Option<String>,
    phone: Option<String>,
}

/// POST /orders
///
/// Multipart handler that ingests a submit-order form plus one or more
/// file parts. Security pipeline:
///
/// 1. session -> student caller
/// 2. `_csrf` field is the FIRST part of the multipart body; we validate
///    it in constant time before reading any subsequent part
/// 3. metadata fields are captured as small text parts
/// 4. each `files` part is streamed through `LocalFileStorage` which
///    enforces size, magic bytes, atomic rename
///
/// On any error after order creation, we cancel the order (cascade
/// deletes file rows) and remove on-disk artifacts.
pub async fn upload_order_handler(
    State(state): State<AppState>,
    session: Session,
    request: Request,
) -> Response
{
    let caller = match resolve_student_caller(&session).await
    {
        Ok(c) => c,
        Err(r) => return r,
    };
    let user_id = match caller
    {
        Caller::Student { user_id } => user_id,
        Caller::Admin => return (StatusCode::FORBIDDEN, "non autorise").into_response(),
    };

    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_default();

    let boundary = match multer::parse_boundary(&content_type)
    {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid content-type").into_response(),
    };

    let body_stream = request.into_body().into_data_stream()
        .map(|r| r.map_err(std::io::Error::other));
    let mut multipart = multer::Multipart::new(body_stream, boundary);

    let mut fields = OrderFormFields::default();
    let mut order_id: Option<i64> = None;
    let mut any_file_uploaded = false;
    let cancel_uc = state.cancel_order.clone();

    // Holds the error to report once we have finished draining the
    // body. Browsers (notably Firefox) reset the connection if we send
    // a response while they are still pushing body bytes, so we must
    // consume the entire multipart even on error before responding.
    enum HandlerError
    {
        Status(StatusCode, &'static str),
        Flash(String),
    }
    let mut deferred_error: Option<HandlerError> = None;

    'outer: while let Some(field_res) = multipart.next_field().await.transpose()
    {
        let mut field = match field_res
        {
            Ok(f) => f,
            Err(e) =>
            {
                eprintln!("upload: multipart parse error: {e}");
                deferred_error = Some(HandlerError::Flash("Envoi invalide.".to_owned()));
                break 'outer;
            }
        };

        let name = field.name().unwrap_or("").to_owned();

        match name.as_str()
        {
            "_csrf" =>
            {
                let v = match read_text_field(&mut field).await
                {
                    Ok(v) => v,
                    Err(_) =>
                    {
                        deferred_error = Some(HandlerError::Status(
                            StatusCode::BAD_REQUEST, "invalid field"));
                        break 'outer;
                    }
                };
                if !validate_csrf_token(&session, &v).await
                {
                    deferred_error = Some(HandlerError::Status(
                        StatusCode::FORBIDDEN, "Invalid CSRF token"));
                    break 'outer;
                }
                fields.csrf = Some(v);
            }
            "software_used" | "material_id" | "quantity" | "comments" | "phone" =>
            {
                if fields.csrf.is_none()
                {
                    deferred_error = Some(HandlerError::Status(
                        StatusCode::FORBIDDEN, "Invalid CSRF token"));
                    break 'outer;
                }
                let v = match read_text_field(&mut field).await
                {
                    Ok(v) => v,
                    Err(_) =>
                    {
                        deferred_error = Some(HandlerError::Status(
                            StatusCode::BAD_REQUEST, "invalid field"));
                        break 'outer;
                    }
                };
                match name.as_str()
                {
                    "software_used" => fields.software_used = Some(v),
                    "material_id" => fields.material_id = Some(v),
                    "quantity" => fields.quantity = Some(v),
                    "comments" => fields.comments = Some(v),
                    "phone" => fields.phone = Some(v),
                    _ => {}
                }
            }
            "files" =>
            {
                if fields.csrf.is_none()
                {
                    deferred_error = Some(HandlerError::Status(
                        StatusCode::FORBIDDEN, "Invalid CSRF token"));
                    break 'outer;
                }

                // The form must send all metadata text fields before
                // the file inputs. If we see a file before
                // software_used is known, the form was tampered with
                // or built wrong.
                if fields.software_used.is_none()
                {
                    deferred_error = Some(HandlerError::Flash(
                        "Formulaire invalide. Veuillez recharger la page.".to_owned()));
                    break 'outer;
                }

                if order_id.is_none()
                {
                    let oid = match create_order_from_fields(&state, user_id, &fields)
                    {
                        Ok(id) => id,
                        Err(e) =>
                        {
                            deferred_error = Some(HandlerError::Flash(
                                user_message(&e).to_owned()));
                            break 'outer;
                        }
                    };
                    order_id = Some(oid);
                }
                let oid = order_id.expect("order_id just set");

                let raw_name = field.file_name().unwrap_or("").to_owned();
                if raw_name.trim().is_empty()
                {
                    continue;
                }

                let mapped = field.map(|r| r.map_err(std::io::Error::other));
                let mut reader = tokio_util::io::StreamReader::new(mapped);
                let res = state.upload_order_file
                    .execute(oid, user_id, &raw_name, &mut reader)
                    .await;

                match res
                {
                    Ok(_) => { any_file_uploaded = true; }
                    Err(e) =>
                    {
                        deferred_error = Some(HandlerError::Flash(
                            user_message(&e).to_owned()));
                        break 'outer;
                    }
                }
            }
            _ =>
            {
                // Unknown field: read and discard so we keep the body
                // stream advancing.
                let _ = read_text_field(&mut field).await;
            }
        }
    }

    // Drain anything the client is still trying to send before we
    // respond. This avoids the "connection reset" error in Firefox.
    while let Ok(Some(mut field)) = multipart.next_field().await
    {
        while let Ok(Some(_)) = field.chunk().await
        {
            // discard
        }
    }

    if let Some(err) = deferred_error
    {
        if let Some(oid) = order_id
        {
            let _ = cancel_uc.execute(oid).await;
        }
        return match err
        {
            HandlerError::Status(code, msg) => (code, msg).into_response(),
            HandlerError::Flash(msg) =>
                flash_and_redirect(&session, FlashLevel::Error, &msg, "/order/new").await,
        };
    }

    let oid = match order_id
    {
        Some(o) => o,
        None =>
        {
            return flash_and_redirect(
                &session,
                FlashLevel::Error,
                "Veuillez joindre au moins un fichier.",
                "/order/new",
            ).await;
        }
    };

    if !any_file_uploaded
    {
        let _ = cancel_uc.execute(oid).await;
        return flash_and_redirect(
            &session,
            FlashLevel::Error,
            "Veuillez joindre au moins un fichier.",
            "/order/new",
        ).await;
    }

    let notify_uc = state.notify_new_order.clone();
    tokio::spawn(async move
    {
        let result = tokio::task::spawn_blocking(move ||
        {
            notify_uc.execute(oid)
        }).await;
        match result
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("notify new order failed: {e}"),
            Err(e) => eprintln!("notify task panicked: {e}"),
        }
    });

    flash_and_redirect(
        &session,
        FlashLevel::Success,
        "Commande envoyee. L'equipe a ete notifiee.",
        "/my-orders",
    ).await
}

fn create_order_from_fields(
    state: &AppState,
    user_id: i64,
    fields: &OrderFormFields,
) -> Result<i64, AppError>
{
    let software_used = fields.software_used.clone().unwrap_or_default();
    let material_id = match fields.material_id.as_deref().unwrap_or("")
    {
        "" => None,
        s => Some(s.parse::<i64>()
            .map_err(|_| AppError::InvalidInput("material_id invalid".to_owned()))?),
    };
    let quantity = fields.quantity.as_deref().unwrap_or("1")
        .parse::<i32>().unwrap_or(1);
    let comments = fields.comments.clone().filter(|s| !s.is_empty());
    let phone = fields.phone.clone().filter(|s| !s.is_empty());

    let input = SubmitOrderInput
    {
        user_id,
        software_used,
        material_id,
        quantity,
        comments,
        phone,
    };
    state.submit_order.execute(input)
}

/// GET /files/:id/download
///
/// Streams the stored bytes to the authorized caller. Non-owners get
/// 404, never 403 -- we never confirm that a file id exists.
pub async fn download_file_handler(
    State(state): State<AppState>,
    session: Session,
    Path(file_id): Path<i64>,
) -> Response
{
    let role: Option<String> = session.get("role").await.ok().flatten();
    let user_id: Option<i64> = session.get("user_id").await.ok().flatten();
    let is_admin = role.as_deref() == Some("admin");

    let file = match state.download_order_file.authorize(file_id, is_admin, user_id)
    {
        Ok(f) => f,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    let fh = match state.storage.open_for_read(&file.stored_filename).await
    {
        Ok(f) => f,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    // Trust the filesystem for the actual size we are about to stream,
    // not the DB value which could diverge if the file was tampered
    // with out of band. On failure fall back to chunked encoding by
    // omitting Content-Length.
    let disk_size = fh.metadata().await.ok().map(|m| m.len());

    let stream = ReaderStream::new(fh);
    let body = Body::from_stream(stream);

    let sanitized = sanitize_download_name(&file.original_filename);
    let cd = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        sanitized.replace('"', ""),
        percent_encode(&file.original_filename),
    );

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, cd)
        .header("Content-Security-Policy", "default-src 'none'; sandbox")
        .header("X-Download-Options", "noopen")
        .header(header::CACHE_CONTROL, "private, no-store, max-age=0")
        .header("Pragma", "no-cache");

    if let Some(size) = disk_size
    {
        builder = builder.header(header::CONTENT_LENGTH, size.to_string());
    }

    builder.body(body)
        .unwrap_or_else(|_|
            (StatusCode::INTERNAL_SERVER_ERROR, "response error").into_response())
}

/// POST /admin/files/:id/delete -- admin only.
pub async fn delete_file_handler(
    State(state): State<AppState>,
    session: Session,
    Path(file_id): Path<i64>,
) -> Response
{
    let role: Option<String> = session.get("role").await.ok().flatten();
    let is_admin = role.as_deref() == Some("admin");
    if !is_admin
    {
        return (StatusCode::FORBIDDEN, "non autorise").into_response();
    }

    // Resolve the order id before deletion so we can redirect the admin
    // back to the order edit page.
    let order_id = state.download_order_file
        .authorize(file_id, true, None)
        .ok()
        .map(|f| f.order_id);

    match state.delete_order_file.execute(file_id, true).await
    {
        Ok(()) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Fichier supprime.").await;
        }
        Err(e) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, user_message(&e)).await;
        }
    }

    let target = order_id
        .map(|id| format!("/admin/order/{id}"))
        .unwrap_or_else(|| "/admin".to_owned());
    Redirect::to(&target).into_response()
}

/// POST /my-orders/:id/cancel -- student cancels their own pending order.
pub async fn student_cancel_order_handler(
    State(state): State<AppState>,
    session: Session,
    Path(order_id): Path<i64>,
) -> Response
{
    let user_id: Option<i64> = session.get("user_id").await.ok().flatten();
    let role: Option<String> = session.get("role").await.ok().flatten();
    let user_id = match (role.as_deref(), user_id)
    {
        (Some("student"), Some(uid)) => uid,
        _ => return (StatusCode::FORBIDDEN, "non autorise").into_response(),
    };

    match state.student_cancel_order.execute(order_id, user_id).await
    {
        Ok(()) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Commande annulee.").await;
        }
        Err(e) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, user_message(&e)).await;
        }
    }
    Redirect::to("/my-orders").into_response()
}

/// POST /admin/order/:id/delete -- admin removes an order entirely.
pub async fn admin_delete_order_handler(
    State(state): State<AppState>,
    session: Session,
    Path(order_id): Path<i64>,
) -> Response
{
    let role: Option<String> = session.get("role").await.ok().flatten();
    if role.as_deref() != Some("admin")
    {
        return (StatusCode::FORBIDDEN, "non autorise").into_response();
    }

    match state.admin_delete_order.execute(order_id, true).await
    {
        Ok(()) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Commande supprimee.").await;
        }
        Err(e) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, user_message(&e)).await;
        }
    }
    Redirect::to("/admin").into_response()
}

/// POST /admin/materials/:id/delete -- admin removes a material.
/// Refuses deletion if the material is referenced by any order.
pub async fn admin_delete_material_handler(
    State(state): State<AppState>,
    session: Session,
    Path(material_id): Path<i64>,
) -> Response
{
    let role: Option<String> = session.get("role").await.ok().flatten();
    if role.as_deref() != Some("admin")
    {
        return (StatusCode::FORBIDDEN, "non autorise").into_response();
    }

    match state.manage_material.delete(material_id)
    {
        Ok(()) =>
        {
            let _ = set_flash(&session, FlashLevel::Success, "Materiau supprime.").await;
        }
        Err(e) =>
        {
            let _ = set_flash(&session, FlashLevel::Error, user_message(&e)).await;
        }
    }
    Redirect::to("/admin/materials").into_response()
}

async fn resolve_student_caller(session: &Session) -> Result<Caller, Response>
{
    let role: Option<String> = session.get("role").await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "session").into_response())?;
    match role.as_deref()
    {
        Some("student") =>
        {
            let user_id: i64 = session.get("user_id").await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "session").into_response())?
                .ok_or_else(|| (StatusCode::UNAUTHORIZED, "unauth").into_response())?;
            Ok(Caller::Student { user_id })
        }
        _ => Err((StatusCode::UNAUTHORIZED, "unauth").into_response()),
    }
}

async fn flash_and_redirect(
    session: &Session,
    level: FlashLevel,
    msg: &str,
    target: &str,
) -> Response
{
    let _ = set_flash(session, level, msg).await;
    Redirect::to(target).into_response()
}

async fn read_text_field(
    field: &mut multer::Field<'_>,
) -> Result<String, String>
{
    const MAX: usize = 8 * 1024;
    let mut buf: Vec<u8> = Vec::with_capacity(128);
    loop
    {
        match field.chunk().await
        {
            Ok(Some(chunk)) =>
            {
                if buf.len().saturating_add(chunk.len()) > MAX
                {
                    return Err("text field too large".to_owned());
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => return Err(format!("multipart read error: {e}")),
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Minimal RFC 3986 percent-encoding for Content-Disposition filename*.
/// Unreserved chars are kept; everything else becomes %XX.
fn percent_encode(s: &str) -> String
{
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes()
    {
        let c = *b;
        let unreserved = c.is_ascii_alphanumeric()
            || c == b'-' || c == b'_' || c == b'.' || c == b'~';
        if unreserved
        {
            out.push(c as char);
        }
        else
        {
            use std::fmt::Write;
            let _ = write!(out, "%{c:02X}");
        }
    }
    out
}
