use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

const DEFAULT_DATABASE_URL: &str = "postgres://sammy:password@127.0.0.1:5432/push_notif";

pub async fn create_pool() -> anyhow::Result<PgPool> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Buat user admin default jika belum ada user.
pub async fn seed_admin_if_empty(pool: &PgPool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count.0 > 0 {
        return Ok(());
    }
    let hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST)?;
    sqlx::query("INSERT INTO users (username, password_hash) VALUES ($1, $2)")
        .bind("admin")
        .bind(&hash)
        .execute(pool)
        .await?;
    tracing::info!("Default user created: admin / admin");
    Ok(())
}
