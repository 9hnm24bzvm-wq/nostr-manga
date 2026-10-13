//! Offline only: no database, HTTP server, signer service or key generation.
use anyhow::{Context, Result, ensure};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action=args.first().context("usage: manga-key-tool verify SOURCE EXPECTED_PUBLIC_KEY | backup SOURCE DESTINATION EXPECTED_PUBLIC_KEY")?;
    let expected = match action.as_str() {
        "verify" => {
            ensure!(
                args.len() == 3,
                "verify requires SOURCE EXPECTED_PUBLIC_KEY"
            );
            manga_node::offline_keys::verify(Path::new(&args[1]), &args[2])?;
            &args[2]
        }
        "backup" => {
            ensure!(
                args.len() == 4,
                "backup requires SOURCE DESTINATION EXPECTED_PUBLIC_KEY"
            );
            manga_node::offline_keys::backup(Path::new(&args[1]), Path::new(&args[2]), &args[3])?;
            &args[3]
        }
        _ => anyhow::bail!("expected verify or backup"),
    };
    println!(
        "{}",
        serde_json::json!({"verified":true,"public_key":expected,"private_key_returned":false,"network_used":false,"membership_restored":false})
    );
    Ok(())
}
