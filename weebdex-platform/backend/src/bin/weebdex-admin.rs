//! One-shot, explicit, offline native administrator bootstrap. Never run at startup.
use anyhow::{Result,bail,ensure};
use std::io::{self,Read,Write};
use std::collections::BTreeMap;
use weebdex_platform::{AppState,Config,domains::{self,staff_authority::bootstrap}};
#[tokio::main]
async fn main()->Result<()>{
 let args:Vec<String>=std::env::args().skip(1).collect();
 if args.first().is_some_and(|v|v=="publication-access"){return publication_access(&args[1..]).await;}
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

/// This operator-controlled command reads configuration/public authority only;
/// it never configures a publication runtime, reads keys or opens source storage.
async fn publication_access(args:&[String])->Result<()>{
 use weebdex_platform::domains::publication_access::cli;
 let Some(action)=args.first() else{bail!("Usage: weebdex-admin publication-access review|apply|cancel|receipt|pending [exact flags]")};
 let allowed:&[&str]=match action.as_str(){"review"=>&["operation","user","publisher","operator","reason"],"apply"|"cancel"|"receipt"=>&["review","digest","operator"],"pending"=>&["user"],_=>bail!("Choose publication-access review, apply, cancel, receipt or pending")};
 ensure!(args.len()==1+allowed.len()*2,"Provide every exact required flag once: {}",allowed.iter().map(|v|format!("--{v}")).collect::<Vec<_>>().join(" "));
 let mut flags=BTreeMap::new();for pair in args[1..].chunks_exact(2){let name=pair[0].strip_prefix("--").ok_or_else(||anyhow::anyhow!("Expected a named flag"))?;ensure!(allowed.contains(&name)&&flags.insert(name,pair[1].as_str()).is_none(),"Unknown or duplicate publication-access flag");}
 for name in allowed{ensure!(flags.contains_key(name),"Missing --{name}");}
 let config=Config::load()?;let state=AppState::connect(config,Config::database_dsn()?).await?;state.db.verify_schema(domains::MIGRATIONS).await?;state.protocol.verify_schema().await?;
 let result=match action.as_str(){
 "review"=>cli::review(&state,flags["operation"],flags["user"],flags["publisher"],flags["operator"],flags["reason"]).await,
 "pending"=>cli::pending(&state,flags["user"]).await,
 "receipt"=>cli::receipt(&state,flags["review"],flags["operator"],flags["digest"]).await,
 "apply"|"cancel"=>{
  let reviewed=cli::inspect(&state,flags["review"],flags["operator"],flags["digest"]).await.map_err(|e|anyhow::anyhow!("{}: {}",e.code,e.message))?;
  println!("{}",serde_json::to_string_pretty(&reviewed)?);
  println!("This changes only dedicated native publish-now access. No original admission, key, signer login, native role, group, schedule or source withdrawal is granted. Operator reference is an audit label, not a verified personal identity.");
  let expected=format!("PUBLICATION-ACCESS {} {} {}",action.to_uppercase(),flags["review"],flags["digest"]);
  println!("Review the exact account, public publisher, revisions, original admission epoch and effect. Type this exact line, or Enter to abort this invocation without settling the review:\n{expected}");io::stdout().flush()?;
  let mut line=Vec::new();for byte in io::stdin().lock().bytes().take(512){let byte=byte?;if byte==b'\n'{break;}line.push(byte);}if line.last()==Some(&b'\r'){line.pop();}
  ensure!(line==expected.as_bytes(),"Confirmation did not match; no access change or cancellation was applied. The review remains unsettled: inspect its exact receipt or cancel deliberately.");
  cli::settle(&state,flags["review"],flags["operator"],flags["digest"],action=="cancel").await
 },_=>unreachable!()}.map_err(|e|anyhow::anyhow!("{}: {}",e.code,e.message))?;
 println!("{}",serde_json::to_string_pretty(&result)?);Ok(())
}
