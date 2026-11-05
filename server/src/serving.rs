//! Private parent/child control channel for the managed image-reader family.
//! A receipt alone never releases a DB hold; exact-set withdrawal remains required.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
#[derive(Clone, Debug)]
pub struct ServingConfig {
    pub python: PathBuf,
    pub helper: PathBuf,
    pub nginx: PathBuf,
    pub runtime: PathBuf,
    pub public: PathBuf,
    pub bind: String,
    pub owner: [u8; 32],
    pub workers: u8,
    pub shutdown_seconds: u8,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReaderProcess {
    pub pid: u32,
    pub birth: u64,
    pub session: u32,
    pub group: u32,
    pub state: String,
}
#[derive(Debug)]
pub struct ReaderDrain {
    workers: Vec<ReaderProcess>,
}
impl ReaderDrain {
    pub fn workers(&self) -> &[ReaderProcess] {
        &self.workers
    }
}
struct Channel {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    usable: bool,
}
struct ControlRequest<'a> {
    channel: &'a mut Channel,
}
impl Drop for ControlRequest<'_> {
    fn drop(&mut self) {
        if !self.channel.usable {
            // Cancellation runs Drop too: EOF tells the supervisor to stop.
            self.channel.input.take();
        }
    }
}
pub struct ManagedServing {
    config: ServingConfig,
    channel: Mutex<Channel>,
    generation: String,
    deadline: Duration,
    owner: [u8; 32],
    public: PathBuf,
}
async fn bounded_line(output: &mut BufReader<ChildStdout>) -> Result<serde_json::Value> {
    let mut line = Vec::new();
    loop {
        let buf = output.fill_buf().await?;
        ensure!(
            !buf.is_empty(),
            "serving supervisor closed its control pipe"
        );
        let n = buf
            .iter()
            .position(|b| *b == b'\n')
            .map_or(buf.len(), |i| i + 1);
        ensure!(
            line.len() + n <= 16384,
            "serving supervisor response budget"
        );
        let complete = buf[n - 1] == b'\n';
        line.extend_from_slice(&buf[..n]);
        output.consume(n);
        if complete {
            return Ok(serde_json::from_slice(&line)?);
        }
    }
}
impl ManagedServing {
    /// Recover the prior family and hold the runtime lock without binding any listener.
    pub async fn prepare(config: ServingConfig) -> Result<Self> {
        ensure!(
            (1..=16).contains(&config.workers) && (1..=120).contains(&config.shutdown_seconds),
            "serving worker/deadline bounds"
        );
        let mut child = Command::new(&config.python)
            // The image reader/helper needs no database credentials or other
            // node secrets from the parent's environment.
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .arg(&config.helper)
            .arg("prepare")
            .arg("--runtime")
            .arg(&config.runtime)
            .arg("--public")
            .arg(&config.public)
            .arg("--nginx")
            .arg(&config.nginx)
            .arg("--bind")
            .arg(&config.bind)
            .arg("--owner")
            .arg(hex::encode(config.owner))
            .arg("--workers")
            .arg(config.workers.to_string())
            .arg("--shutdown-seconds")
            .arg(config.shutdown_seconds.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            // Dropping the stdin pipe triggers orderly family shutdown. Killing
            // only the helper would leave reader descendants alive.
            .kill_on_drop(false)
            .spawn()?;
        let input = child.stdin.take().context("serving stdin missing")?;
        let output = child.stdout.take().context("serving stdout missing")?;
        let mut channel = Channel {
            child,
            input: Some(input),
            output: BufReader::new(output),
            usable: true,
        };
        let deadline = Duration::from_secs(u64::from(config.shutdown_seconds) + 12);
        let ready = tokio::time::timeout(deadline, bounded_line(&mut channel.output)).await??;
        ensure!(
            ready.get("ready").and_then(|x| x.as_bool()) == Some(true),
            "serving startup failed"
        );
        let generation = ready
            .get("generation")
            .and_then(|x| x.as_str())
            .context("serving generation missing")?
            .to_owned();
        ensure!(
            generation.len() == 32 && generation.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid serving generation"
        );
        Ok(Self {
            config: config.clone(),
            channel: Mutex::new(channel),
            generation,
            deadline,
            owner: config.owner,
            public: config.public.canonicalize()?,
        })
    }
    async fn request(&self, op: &str) -> Result<serde_json::Value> {
        let mut c = self.channel.lock().await;
        ensure!(c.usable, "serving channel requires recovery");
        // Poison before await: cancellation must not let a delayed old response
        // be consumed as a new drain receipt. Closing on error triggers EOF.
        c.usable = false;
        let mut guard = ControlRequest { channel: &mut c };
        let c = &mut guard.channel;
        let nonce = hex::encode(rand::random::<[u8; 32]>());
        let mut request = serde_json::to_vec(&serde_json::json!({"op":op,"nonce":nonce}))?;
        request.push(b'\n');
        let result = tokio::time::timeout(self.deadline, async {
            let input = c.input.as_mut().context("serving control closed")?;
            input.write_all(&request).await?;
            input.flush().await?;
            let response = bounded_line(&mut c.output).await?;
            ensure!(
                response.get("nonce").and_then(|v| v.as_str()) == Some(nonce.as_str()),
                "serving response nonce mismatch"
            );
            ensure!(
                response.get("ok").and_then(|v| v.as_bool()) == Some(true),
                "serving request failed: {}",
                response.get("error").unwrap_or(&serde_json::Value::Null)
            );
            Ok::<_, anyhow::Error>(
                response
                    .get("result")
                    .context("serving result absent")?
                    .clone(),
            )
        })
        .await;
        match result {
            Ok(Ok(value)) => {
                c.usable = true;
                Ok(value)
            }
            other => {
                c.input.take();
                match other {
                    Ok(Err(error)) => Err(error),
                    Err(error) => Err(error.into()),
                    _ => unreachable!(),
                }
            }
        }
    }
    pub async fn check_active(&self) -> Result<()> {
        let result = self.request("status").await?;
        ensure!(
            result.get("generation").and_then(|v| v.as_str()) == Some(self.generation.as_str()),
            "serving status generation mismatch"
        );
        ensure!(
            result.get("active").and_then(|v| v.as_bool()) == Some(true),
            "serving master unavailable"
        );
        let members = result
            .get("workers")
            .and_then(|v| v.as_array())
            .context("serving process status missing")?;
        ensure!(
            members.len() >= 2 && members.len() <= 33,
            "serving master/worker family incomplete"
        );
        Ok(())
    }
    /// Recover orphaned readers after a dead/poisoned helper. This is independent
    /// of the database and never activates a replacement listener.
    pub async fn shutdown_verified(&self) -> Result<()> {
        if self.stop().await.is_ok() {
            return Ok(());
        }
        {
            let mut channel = self.channel.lock().await;
            channel.usable = false;
            channel.input.take();
            tokio::time::timeout(self.deadline, channel.child.wait()).await??;
        }
        let recovered = Self::prepare(self.config.clone()).await?;
        recovered.stop().await
    }
    pub async fn activate(&self) -> Result<()> {
        let response = self.request("activate").await?;
        ensure!(
            response.get("active").and_then(|v| v.as_bool()) == Some(true)
                && response.get("generation").and_then(|v| v.as_str())
                    == Some(self.generation.as_str()),
            "serving activation mismatch"
        );
        Ok(())
    }
    pub fn operation_deadline(&self) -> Duration {
        self.deadline + Duration::from_secs(10)
    }
    pub fn owner(&self) -> [u8; 32] {
        self.owner
    }
    pub fn public_path(&self) -> &std::path::Path {
        &self.public
    }
    pub async fn drain(&self) -> Result<ReaderDrain> {
        let response = self.request("drain").await?;
        ensure!(
            response.get("drained").and_then(|v| v.as_bool()) == Some(true)
                && response.get("generation").and_then(|v| v.as_str())
                    == Some(self.generation.as_str()),
            "serving drain generation mismatch"
        );
        let workers: Vec<ReaderProcess> = serde_json::from_value(
            response
                .get("workers")
                .context("drained workers absent")?
                .clone(),
        )?;
        ensure!(
            (!workers.is_empty()
                || response.get("inactive").and_then(|v| v.as_bool()) == Some(true))
                && workers.len() <= 32,
            "serving drain worker bounds"
        );
        Ok(ReaderDrain { workers })
    }
    async fn stop(&self) -> Result<()> {
        self.request("stop").await?;
        let mut c = self.channel.lock().await;
        c.usable = false;
        c.input.take();
        ensure!(
            tokio::time::timeout(self.deadline, c.child.wait())
                .await??
                .success(),
            "serving supervisor shutdown failed"
        );
        Ok(())
    }
}
