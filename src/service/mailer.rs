use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::env;
use tracing::{info, warn};

pub struct MailerService {
    smtp_host: Option<String>,
    smtp_port: Option<u16>,
    smtp_user: Option<String>,
    smtp_password: Option<String>,
    alert_from: Option<String>,
    alert_to: Option<String>,
}

impl MailerService {
    pub fn from_env() -> Self {
        let smtp_host = env::var("SMTP_HOST").ok();
        let smtp_port = env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok());
        let smtp_user = env::var("SMTP_USER").ok();
        let smtp_password = env::var("SMTP_PASSWORD").ok();
        let alert_from = env::var("ALERT_EMAIL_FROM").ok();
        let alert_to = env::var("ALERT_EMAIL_TO").ok();

        Self {
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_password,
            alert_from,
            alert_to,
        }
    }

    pub async fn send_error_alert(
        &self,
        recipe_id: &str,
        recipe_title: &str,
        error_msg: &str,
        retry_count: i64,
    ) -> Result<(), String> {
        let (host, user, password, from, to) = match (
            &self.smtp_host,
            &self.smtp_user,
            &self.smtp_password,
            &self.alert_from,
            &self.alert_to,
        ) {
            (Some(h), Some(u), Some(p), Some(f), Some(t)) => (h, u, p, f, t),
            _ => {
                info!(
                    "SMTP alerting is not fully configured. Logging recipe audit error locally:\n\
                     Recipe ID: {}\n\
                     Recipe Title: {}\n\
                     Error: {}\n\
                     Retry Count: {}",
                    recipe_id, recipe_title, error_msg, retry_count
                );
                return Ok(());
            }
        };

        let port = self.smtp_port.unwrap_or(587);

        let from_mailbox = from
            .parse::<Mailbox>()
            .map_err(|e| format!("Failed to parse ALERT_EMAIL_FROM: {}", e))?;
        let to_mailbox = to
            .parse::<Mailbox>()
            .map_err(|e| format!("Failed to parse ALERT_EMAIL_TO: {}", e))?;

        let subject = format!(
            "Tagatoni Alert: Failed to audit recipe \"{}\"",
            recipe_title
        );
        let body = format!(
            "Hello,\n\n\
             The Tagatoni agent failed to audit a recipe after exhausting retries.\n\n\
             Recipe Details:\n\
             - ID: {}\n\
             - Title: {}\n\
             - Last Error: {}\n\
             - Total Retry Attempts: {}\n\n\
             Please check the agent logs and database state.\n\n\
             Best regards,\n\
             Tagatoni Daemon",
            recipe_id, recipe_title, error_msg, retry_count
        );

        let email = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .body(body)
            .map_err(|e| format!("Failed to construct email: {}", e))?;

        let creds = Credentials::new(user.clone(), password.clone());
        let transport = if port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                .map_err(|e| format!("SMTP host error: {}", e))?
                .credentials(creds)
                .port(port)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| format!("SMTP host error: {}", e))?
                .credentials(creds)
                .port(port)
                .build()
        };

        info!("Sending recipe audit error alert email to {}...", to);
        match transport.send(email).await {
            Ok(_) => {
                info!("Alert email sent successfully.");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to send alert email: {}", e);
                Err(format!("SMTP transport error: {}", e))
            }
        }
    }
}
