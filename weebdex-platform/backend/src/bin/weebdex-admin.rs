//! One-shot, explicit, offline native administrator bootstrap. Never run at startup.
use anyhow::{Result,bail,ensure};
use std::io::{self,Write};
use weebdex_platform::{AppState,Config,domains::{self,staff_authority::bootstrap}};
#[tokio::main]
async fn main()->Result<()>{
 let args:Vec<String>=std::env::args().skip(1).collect();
 ensure!(args.len()==5&&args[0]=="bootstrap"&&args[1]=="--user"&&args[3]=="--reason","Usage: weebdex-admin bootstrap --user EXISTING_NATIVE_ACCOUNT_ID --reason 'reviewable reason'");
 let config=Config::load()?;let state=AppState::connect(config,Config::database_dsn()?).await?;
 state.db.verify_schema(domains::MIGRATIONS).await?;
 let preview=bootstrap::preview(&state.db,&args[2],&args[4]).await.map_err(|e|anyhow::anyhow!("{}: {}",e.code,e.message))?;
 println!("{}",serde_json::to_string_pretty(&preview)?);
 let digest=preview["digest"].as_str().ok_or_else(||anyhow::anyhow!("Bootstrap review digest missing"))?;
 let expected=format!("BOOTSTRAP {} {}",args[2],digest);
 println!("Review every displayed local role and grant. No password, email, API secret or operator key is displayed.");
 println!("Type the following exact confirmation, or press Enter to cancel:\n{expected}");io::stdout().flush()?;
 let mut input=String::new();io::stdin().read_line(&mut input)?;
 if input.trim_end_matches(['\r','\n'])!=expected{bail!("Bootstrap canceled; no authority was changed");}
 let receipt=bootstrap::apply(&state.db,&args[2],&args[4],digest).await.map_err(|e|anyhow::anyhow!("{}: {}",e.code,e.message))?;
 println!("{}",serde_json::to_string_pretty(&receipt)?);Ok(())
}
