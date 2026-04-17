use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::SmtpTransport;
use lettre::{Message, Transport};

use crate::domain::errors::DomainError;
use crate::domain::notifications::OrderNotification;
use crate::domain::repositories::NotificationSender;

use super::config::EmailConfig;

pub struct SmtpNotificationSender
{
    config: EmailConfig,
    transport: SmtpTransport,
}

impl SmtpNotificationSender
{
    pub fn new(config: EmailConfig) -> Result<Self, DomainError>
    {
        let creds = Credentials::new(
            config.smtp_user.clone(),
            config.smtp_password.clone(),
        );

        let transport = SmtpTransport::relay(&config.smtp_host)
            .map_err(|e| DomainError::Validation(format!("smtp relay setup: {e}")))?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        Ok(Self { config, transport })
    }
}

impl NotificationSender for SmtpNotificationSender
{
    fn notify_new_order(&self, n: &OrderNotification) -> Result<(), DomainError>
    {
        let subject = format!("Nouvelle commande #{} - {}", n.order_id, n.user_display_name);
        let body = render_html(n);

        let from = self.config.from_address
            .parse()
            .map_err(|e| DomainError::Validation(format!("invalid SMTP_FROM: {e}")))?;
        let to = self.config.admin_address
            .parse()
            .map_err(|e| DomainError::Validation(format!("invalid ADMIN_EMAIL: {e}")))?;

        let email = Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body)
            .map_err(|e| DomainError::Validation(format!("email build error: {e}")))?;

        self.transport
            .send(&email)
            .map_err(|e| DomainError::Validation(format!("smtp send error: {e}")))?;

        Ok(())
    }
}

fn escape_html(s: &str) -> String
{
    s.chars()
        .map(|c| match c
        {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            '\'' => "&#39;".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

fn render_html(n: &OrderNotification) -> String
{
    let material = n.material_label.as_deref().unwrap_or("-");
    let phone = n.user_phone.as_deref().unwrap_or("-");
    let comments = n.comments.as_deref().unwrap_or("-");

    let file_links_html = n.files
        .iter()
        .map(|f|
        {
            let name = escape_html(&f.original_filename);
            let url = format!("{}/admin/files/{}/download", n.download_base_url.trim_end_matches('/'), f.file_id);
            let safe_url = escape_html(&url);
            let size_kb = (f.size_bytes + 1023) / 1024;
            format!(r#"<li style="margin-bottom:6px;"><a href="{safe_url}" style="color:#6E84D8;word-break:break-all;">{name}</a> <span style="color:#8A8A84;font-size:12px;">({size_kb} KB)</span></li>"#)
        })
        .collect::<Vec<_>>()
        .join("");

    let row = |label: &str, value: &str| -> String
    {
        format!
        (
            r#"<tr><td style="padding:10px 12px;background:#232327;color:#B5B5B0;font-weight:500;font-size:13px;border-bottom:1px solid #2C2C30;width:140px;">{label}</td><td style="padding:10px 12px;color:#F2F2F0;font-size:14px;border-bottom:1px solid #2C2C30;">{value}</td></tr>"#
        )
    };

    format!
    (
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Nouvelle commande</title></head>
<body style="font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,sans-serif;background:#121212;color:#F2F2F0;margin:0;padding:24px;">
  <div style="max-width:620px;margin:0 auto;background:#1B1B1E;border:1px solid #2C2C30;border-radius:14px;overflow:hidden;">
    <div style="padding:28px 32px 20px;border-bottom:1px solid #2C2C30;">
      <div style="display:inline-block;background:#2F4798;color:#FFFFFF;font-size:11px;font-weight:700;padding:4px 8px;border-radius:6px;letter-spacing:0.03em;">FABLAB</div>
      <h1 style="margin:12px 0 4px;font-size:22px;color:#F2F2F0;font-weight:700;letter-spacing:-0.02em;">Nouvelle commande #{id}</h1>
      <p style="margin:0;color:#B5B5B0;font-size:14px;">Une nouvelle commande vient d'etre deposee sur le portail.</p>
    </div>
    <div style="padding:24px 32px;">
      <table cellpadding="0" cellspacing="0" border="0" style="border-collapse:collapse;width:100%;border:1px solid #2C2C30;border-radius:10px;overflow:hidden;">
        {rows}
      </table>
      <h3 style="margin:24px 0 10px;font-size:14px;color:#B5B5B0;font-weight:600;">Fichiers</h3>
      <ul style="padding-left:20px;margin:0;font-size:14px;">{files}</ul>
    </div>
    <div style="padding:16px 32px;background:#232327;color:#8A8A84;font-size:12px;text-align:center;">
      Message automatique. Ne pas repondre.
    </div>
  </div>
</body></html>"#,
        id = n.order_id,
        rows = [
            row("Date", &escape_html(&n.created_at)),
            row("Client", &escape_html(&n.user_display_name)),
            row("Email", &escape_html(&n.user_email)),
            row("Telephone", &escape_html(phone)),
            row("Logiciel", &escape_html(&n.software_used)),
            row("Materiau", &escape_html(material)),
            row("Quantite", &n.quantity.to_string()),
            row("Commentaires", &escape_html(comments)),
        ].join(""),
        files = file_links_html,
    )
}