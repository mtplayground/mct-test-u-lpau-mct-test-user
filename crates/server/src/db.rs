use sqlx::{
    migrate::MigrateError,
    postgres::{PgPool, PgPoolOptions},
    Error as SqlxError,
};

use crate::config::Config;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(config: &Config) -> Result<Self, DatabaseError> {
        let pool = PgPoolOptions::new()
            .connect(&config.database_url)
            .await
            .map_err(DatabaseError::Connect)?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(DatabaseError::Migrate)?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Debug)]
pub enum DatabaseError {
    Connect(SqlxError),
    Migrate(MigrateError),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(error) => write!(f, "failed to connect to Postgres: {error}"),
            Self::Migrate(error) => write!(f, "failed to run database migrations: {error}"),
        }
    }
}

impl std::error::Error for DatabaseError {}
