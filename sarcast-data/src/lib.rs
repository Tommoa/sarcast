#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(unused_results)]
#![deny(unreachable_pub)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![deny(bad_style)]
#![deny(unused)]
#![deny(clippy::pedantic)]

use diesel::migration::MigrationSource;
use diesel::prelude::*;
use diesel::{r2d2, r2d2::ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use once_cell::sync::Lazy;
use std::path::PathBuf;

/// The database models used for sarcast
pub mod models;
#[allow(missing_docs)]
pub mod schema;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

static POOL: Lazy<Pool> = Lazy::new(|| init_pool(DB_PATH.to_str().unwrap()));

/// TODO: Change this depending on the platform
static DB_PATH: Lazy<PathBuf> = Lazy::new(|| "./podcasts.db".into());

/// The embedded set of migrations for this version of sarcast
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

/// Get an r2d2 `SqliteConnection`
pub fn connection() -> Pool {
    POOL.clone()
}

fn init_pool(db_path: &str) -> Pool {
    let manager = ConnectionManager::<SqliteConnection>::new(db_path);
    let pool = r2d2::Pool::builder()
        .max_size(1)
        .build(manager)
        .expect("Failed to create pool.");

    {
        let mut db = pool.get().expect("Failed to initialize pool.");
        run_migration_on(&mut *db).expect("Failed to run migrations during init.");
    }
    pool
}

fn run_migration_on(connection: &mut SqliteConnection) -> Result<(), String> {
    for migration in MIGRATIONS.migrations().map_err(|e| format!("{}", e))? {
        migration.run(connection).map_err(|e| format!("{}", e))?;
    }
    Ok(())
}

/// Reset the database into a clean state
pub fn truncate_db() -> Result<(), String> {
    use diesel::connection::SimpleConnection;
    let db = connection();
    let mut con = db.get().map_err(|e| format!("{}", e))?;
    con.batch_execute("DELETE FROM episodes")
        .map_err(|e| format!("{}", e))?;
    con.batch_execute("DELETE FROM shows")
        .map_err(|e| format!("{}", e))?;
    con.batch_execute("DELETE FROM source")
        .map_err(|e| format!("{}", e))?;
    Ok(())
}
