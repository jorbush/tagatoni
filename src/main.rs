mod model;
mod service;

use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client};
use service::gemini::GeminiService;
use service::mailer::MailerService;
use service::mongo::MongoService;
use service::turso::TursoService;
use std::env;
use std::time::Duration;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set up tracing subscriber");

    info!("Starting Tagatoni Recipe Audit Agent 🏷️");

    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| "jorbites".to_string());
    let gemini_api_key = env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY must be set");
    let turso_url = env::var("TURSO_URL").expect("TURSO_URL must be set");
    let turso_token = env::var("TURSO_AUTH_TOKEN").expect("TURSO_AUTH_TOKEN must be set");

    let sleep_between_recipes_secs = env::var("SLEEP_BETWEEN_RECIPES_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60);

    let idle_sleep_secs = env::var("IDLE_SLEEP_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(600);

    let max_retries = env::var("MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(3);

    let retry_cooldown_hours = env::var("RETRY_COOLDOWN_HOURS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(24);

    info!("Connecting to MongoDB...");
    let mongo_options = ClientOptions::parse(&mongodb_uri).await?;
    let mongo_client = Client::with_options(mongo_options)?;
    let mongo_service = MongoService::new(mongo_client, db_name);

    info!("Connecting to Turso...");
    let turso_service = TursoService::new(&turso_url, &turso_token).await?;

    let gemini_service = GeminiService::new(gemini_api_key);
    let mailer_service = MailerService::from_env();

    info!("Services initialized. Starting audit loop.");

    loop {
        // 1. Fetch next recipe to audit
        let fetch_result = mongo_service.fetch_next_recipe_to_audit().await;
        let recipe = match fetch_result {
            Ok(Some(r)) => r,
            Ok(None) => {
                info!(
                    "No recipes to audit. Sleeping for {} seconds...",
                    idle_sleep_secs
                );
                tokio::time::sleep(Duration::from_secs(idle_sleep_secs)).await;
                continue;
            }
            Err(e) => {
                error!(
                    "Error fetching recipe from MongoDB: {}. Retrying in 10s...",
                    e
                );
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        let recipe_id_str = recipe.id.to_hex();
        let recipe_title = recipe.title.clone();

        // 2. Check Turso to see if it needs processing
        match turso_service
            .needs_processing(&recipe_id_str, max_retries, retry_cooldown_hours)
            .await
        {
            Ok(true) => {
                info!("Processing recipe: {} ({})", recipe_title, recipe_id_str);
            }
            Ok(false) => {
                // If it is not ready for processing (already ok/skipped, or cooldown active),
                // we skip it. Sleep a tiny bit (2 seconds) to avoid CPU thrashing if MongoDB keeps returning it.
                debug!(
                    "Recipe {} ({}) does not need processing. Skipping.",
                    recipe_title, recipe_id_str
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            Err(e) => {
                error!("Error querying Turso database: {}. Retrying in 10s...", e);
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        }

        // 3. Double check if both fields are non-null in MongoDB
        if recipe.calories.is_some() && recipe.recipe_cuisine.is_some() {
            info!(
                "Recipe {} ({}) already has both fields populated in MongoDB. Marking as skipped.",
                recipe_title, recipe_id_str
            );
            if let Err(e) = turso_service.mark_skipped(&recipe_id_str).await {
                error!(
                    "Failed to mark recipe {} as skipped in Turso: {}",
                    recipe_id_str, e
                );
            }
            continue;
        }

        // 4. Call Gemini Interactions API with exponential backoff for transient errors
        let mut gemini_result = None;
        let mut backoff_secs = 5;
        for attempt in 1..=3 {
            match gemini_service.audit_recipe(&recipe).await {
                Ok(res) => {
                    gemini_result = Some(res);
                    break;
                }
                Err(e) => {
                    warn!(
                        "Gemini API attempt {} failed for recipe {}: {}.",
                        attempt, recipe_id_str, e
                    );
                    if attempt < 3 {
                        info!("Backing off for {} seconds before retry...", backoff_secs);
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                        backoff_secs *= 2;
                    }
                }
            }
        }

        let audit_result = match gemini_result {
            Some(res) => res,
            None => {
                let err_msg = "Gemini API call failed after 3 attempts with exponential backoff";
                error!("{}, marking recipe {} as error.", err_msg, recipe_id_str);

                if let Err(e) = turso_service.mark_error(&recipe_id_str, err_msg).await {
                    error!("Failed to record error in Turso: {}", e);
                }

                let _ = mailer_service
                    .send_error_alert(&recipe_id_str, &recipe_title, err_msg, max_retries)
                    .await;

                tokio::time::sleep(Duration::from_secs(sleep_between_recipes_secs)).await;
                continue;
            }
        };

        // 5. Update MongoDB
        info!(
            "Audit successful for recipe {}: Calories = {}, Cuisine = {}. Updating MongoDB...",
            recipe_title, audit_result.calories, audit_result.recipe_cuisine
        );
        match mongo_service
            .update_recipe_seo(
                &recipe.id,
                audit_result.calories,
                &audit_result.recipe_cuisine,
            )
            .await
        {
            Ok(_) => {
                info!("MongoDB updated successfully for recipe {}.", recipe_title);
                if let Err(e) = turso_service.mark_ok(&recipe_id_str).await {
                    error!(
                        "Failed to mark recipe {} as OK in Turso: {}",
                        recipe_id_str, e
                    );
                }
            }
            Err(e) => {
                let err_msg = format!("Failed to update recipe in MongoDB: {}", e);
                error!("{}", err_msg);
                if let Err(te) = turso_service.mark_error(&recipe_id_str, &err_msg).await {
                    error!("Failed to record MongoDB update error in Turso: {}", te);
                }
                let _ = mailer_service
                    .send_error_alert(&recipe_id_str, &recipe_title, &err_msg, 1)
                    .await;
            }
        }

        // 6. Sleep between recipes
        info!(
            "Sleeping for {} seconds before the next recipe...",
            sleep_between_recipes_secs
        );
        tokio::time::sleep(Duration::from_secs(sleep_between_recipes_secs)).await;
    }
}
