use libsql::{params, Database};

pub struct TursoService {
    db: Database,
}

impl TursoService {
    pub async fn new(url: &str, token: &str) -> Result<Self, libsql::Error> {
        let db = libsql::Builder::new_remote(url.to_string(), token.to_string())
            .build()
            .await?;

        let conn = db.connect()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audited_recipes (
                recipe_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_attempt TEXT NOT NULL,
                error_msg TEXT
            )",
            (),
        )
        .await?;

        Ok(Self { db })
    }

    /// Determines if a recipe needs auditing or a retry.
    /// Returns true if it has never been processed, OR if it errored and is ready for retry.
    pub async fn needs_processing(
        &self,
        recipe_id: &str,
        max_retries: i64,
        cooldown_hours: i64,
    ) -> Result<bool, libsql::Error> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query(
                "SELECT status, retry_count, last_attempt FROM audited_recipes WHERE recipe_id = ?1",
                params![recipe_id],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let status: String = row.get(0)?;
            let retry_count: i64 = row.get(1)?;
            let last_attempt_str: String = row.get(2)?;

            if status == "ok" || status == "skipped" {
                return Ok(false);
            }

            if status == "error" {
                if retry_count >= max_retries {
                    return Ok(false); // Retries exhausted
                }

                // Check retry cooldown
                if let Ok(last_attempt) = chrono::DateTime::parse_from_rfc3339(&last_attempt_str) {
                    let now = chrono::Utc::now();
                    let duration =
                        now.signed_duration_since(last_attempt.with_timezone(&chrono::Utc));
                    if duration.num_hours() < cooldown_hours {
                        return Ok(false); // Cooldown hasn't passed
                    }
                }
                return Ok(true); // Ready for retry
            }

            Ok(false)
        } else {
            Ok(true) // Never processed
        }
    }

    pub async fn mark_ok(&self, recipe_id: &str) -> Result<(), libsql::Error> {
        let conn = self.db.connect()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audited_recipes (recipe_id, status, retry_count, last_attempt, error_msg)
             VALUES (?1, 'ok', 0, ?2, NULL)
             ON CONFLICT(recipe_id) DO UPDATE SET
                status = 'ok',
                retry_count = 0,
                last_attempt = ?2,
                error_msg = NULL",
            params![recipe_id, now],
        )
        .await?;
        Ok(())
    }

    pub async fn mark_skipped(&self, recipe_id: &str) -> Result<(), libsql::Error> {
        let conn = self.db.connect()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audited_recipes (recipe_id, status, retry_count, last_attempt, error_msg)
             VALUES (?1, 'skipped', 0, ?2, NULL)
             ON CONFLICT(recipe_id) DO UPDATE SET
                status = 'skipped',
                retry_count = 0,
                last_attempt = ?2,
                error_msg = NULL",
            params![recipe_id, now],
        )
        .await?;
        Ok(())
    }

    pub async fn mark_error(&self, recipe_id: &str, error_msg: &str) -> Result<(), libsql::Error> {
        let conn = self.db.connect()?;
        let now = chrono::Utc::now().to_rfc3339();

        // Get current retry count
        let mut rows = conn
            .query(
                "SELECT retry_count FROM audited_recipes WHERE recipe_id = ?1",
                params![recipe_id],
            )
            .await?;

        let new_retry_count = if let Some(row) = rows.next().await? {
            let current_count: i64 = row.get(0)?;
            current_count + 1
        } else {
            1
        };

        conn.execute(
            "INSERT INTO audited_recipes (recipe_id, status, retry_count, last_attempt, error_msg)
             VALUES (?1, 'error', ?2, ?3, ?4)
             ON CONFLICT(recipe_id) DO UPDATE SET
                status = 'error',
                retry_count = ?2,
                last_attempt = ?3,
                error_msg = ?4",
            params![recipe_id, new_retry_count, now, error_msg],
        )
        .await?;
        Ok(())
    }
}
