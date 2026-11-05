use anyhow::{Context, Result};
use manga_node::node::{NodeConfig, open_repository, serve_prepared};
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut args = std::env::args().skip(1);
    let action = args
        .next()
        .context("usage: manga-node migrate|serve CONFIG")?;
    let path = args.next().context("configuration path required")?;
    anyhow::ensure!(args.next().is_none(), "unexpected command argument");
    let config: NodeConfig =
        serde_json::from_slice(&std::fs::read(path).context("cannot read node configuration")?)?;
    let dsn = std::env::var("MANGA_DATABASE_URL").context("MANGA_DATABASE_URL is required")?;
    let serving = if action == "serve" {
        manga_node::serving_role::ServingRole::prepare(&config).await?
    } else {
        None
    };
    let repo = open_repository(&config, dsn).await?;
    match action.as_str() {
        "migrate" => repo.migrate().await,
        "serve" => serve_prepared(config, repo, serving).await,
        _ => anyhow::bail!("expected migrate or serve"),
    }
}
