//! CLI entry point for `server::demo_seed` - see that module for what actually gets seeded.
//!
//! Usage: `cargo run --bin seed [--include-admin-data]`
//!
//! `--include-admin-data` additionally seeds demo accounts/characters/audit-log rows so the
//! admin panel isn't empty in local dev/QA. It's never passed in production (see
//! `docker-compose.prod.yml`'s demo-reset sidecar) - those rows aren't meant to be reachable by
//! the public, anonymous-token demo group at all.
use server::config::Config;
use server::db;
use server::demo_seed;
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let include_admin_data = std::env::args().any(|arg| arg == "--include-admin-data");

    let config = Config::from_env()?;
    let pool = config.pg.create_pool(None, NoTls)?;
    let mut client = pool.get().await?;

    // The sidecar that runs this on a schedule (see docker-compose.prod.yml) can start before
    // the main server has ever run once, so make sure the schema exists rather than assuming it.
    db::update_schema(&mut client).await?;

    let group_id = demo_seed::run(&mut client, include_admin_data).await?;
    log::info!("Demo group (group_id={}) seeded", group_id);
    Ok(())
}
