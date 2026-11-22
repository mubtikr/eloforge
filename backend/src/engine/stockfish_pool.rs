use std::{
    collections::BTreeMap,
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use async_channel::{bounded, Receiver, Sender};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::oneshot,
    time::timeout,
};

use crate::{config::Config, metrics::Metrics};

#[derive(Debug, Clone)]
pub struct EngineQuery {
    pub fen: String,
    pub depth: u8,
    pub multipv: usize,
    pub uci_elo: Option<u32>,
    pub movetime_ms: Option<u64>,
}

impl EngineQuery {
    pub fn new(fen: impl Into<String>, depth: u8, multipv: usize) -> Self {
        Self {
            fen: fen.into(),
            depth,
            multipv: multipv.max(1),
            uci_elo: None,
            movetime_ms: None,
        }
    }

    pub fn with_elo(mut self, elo: u32) -> Self {
        self.uci_elo = Some(elo.clamp(1320, 3190));
        self
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineLine {
    pub multipv: usize,
    pub depth: u8,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub pv: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineEval {
    pub best_move: Option<String>,
    pub depth: u8,
    pub lines: Vec<EngineLine>,
}

impl EngineEval {
    pub fn top(&self) -> Option<&EngineLine> {
        self.lines.first()
    }

    pub fn best_pv(&self) -> Vec<String> {
        self.top().map(|l| l.pv.clone()).unwrap_or_default()
    }
}

type Job = (EngineQuery, oneshot::Sender<Result<EngineEval, String>>);

pub struct EnginePool {
    tx: Sender<Job>,
    workers: usize,
    timeout: Duration,
    metrics: Arc<Metrics>,
    generation: AtomicUsize,
}

impl std::fmt::Debug for EnginePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnginePool")
            .field("workers", &self.workers)
            .field("queue_depth", &self.tx.len())
            .finish()
    }
}

impl EnginePool {
    pub fn new(cfg: &Config, metrics: Arc<Metrics>) -> Arc<Self> {
        let workers = cfg.stockfish_workers.max(1);
        let (tx, rx) = bounded::<Job>(workers * 8);
        let timeout = Duration::from_millis(cfg.stockfish_timeout_ms.clamp(500, 30_000));

        metrics.stockfish_workers.set(workers as i64);

        for _ in 0..workers {
            let rx = rx.clone();
            let path = cfg.stockfish_path.clone();
            let hash = cfg.stockfish_hash_mb;
            let metrics = metrics.clone();
            tokio::spawn(async move {
                worker_loop(path, hash, timeout, rx, metrics).await;
            });
        }

        Arc::new(Self {
            tx,
            workers,
            timeout,
            metrics,
            generation: AtomicUsize::new(0),
        })
    }

    pub fn worker_count(&self) -> usize {
        self.workers
    }

    pub fn queue_depth(&self) -> usize {
        self.tx.len()
    }

    pub async fn evaluate(&self, query: EngineQuery) -> anyhow::Result<EngineEval> {
        self.metrics.stockfish_queue_depth.set(self.tx.len() as i64);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send((query, reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("stockfish pool is shut down"))?;

        let wait = self.timeout + Duration::from_secs(3);
        match timeout(wait, reply_rx).await {
            Ok(Ok(Ok(eval))) => Ok(eval),
            Ok(Ok(Err(err))) => {
                self.metrics.stockfish_errors.inc();
                Err(anyhow::anyhow!(err))
            }
            Ok(Err(_)) => {
                self.metrics.stockfish_errors.inc();
                Err(anyhow::anyhow!("stockfish worker dropped the request"))
            }
            Err(_) => {
                self.metrics.stockfish_errors.inc();
                Err(anyhow::anyhow!("stockfish query timed out"))
            }
        }
    }

    pub async fn ping(&self) -> bool {
        self.generation.fetch_add(1, Ordering::Relaxed);
        let query = EngineQuery {
            fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            depth: 1,
            multipv: 1,
            uci_elo: None,
            movetime_ms: Some(300),
        };
        self.evaluate(query).await.is_ok()
    }
}

struct EngineProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl EngineProcess {
    async fn spawn(path: &str, hash_mb: u32) -> anyhow::Result<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to spawn Stockfish at {path}"))?;
        let stdin = child.stdin.take().context("stockfish stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("stockfish stdout unavailable")?;
        let mut proc = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };

        proc.write("uci").await?;
        proc.wait_for(&["uciok"]).await?;
        proc.write(&format!("setoption name Hash value {hash_mb}"))
            .await?;
        proc.write("setoption name Threads value 1").await?;
        proc.write("isready").await?;
        proc.wait_for(&["readyok"]).await?;
        Ok(proc)
    }

    async fn write(&mut self, line: &str) -> anyhow::Result<()> {
        self.stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .context("failed writing to stockfish")?;
        self.stdin
            .flush()
            .await
            .context("failed flushing stockfish")?;
        Ok(())
    }

    async fn wait_for(&mut self, needles: &[&str]) -> anyhow::Result<()> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .await
                .context("failed reading stockfish output")?;
            if n == 0 {
                anyhow::bail!("stockfish closed its output stream");
            }
            let trimmed = line.trim();
            if needles.contains(&trimmed) {
                return Ok(());
            }
        }
    }

    async fn evaluate(&mut self, query: &EngineQuery) -> anyhow::Result<EngineEval> {
        if let Some(elo) = query.uci_elo {
            self.write("setoption name UCI_LimitStrength value true")
                .await?;
            self.write(&format!("setoption name UCI_Elo value {elo}"))
                .await?;
        } else {
            self.write("setoption name UCI_LimitStrength value false")
                .await?;
        }
        self.write(&format!(
            "setoption name MultiPV value {}",
            query.multipv.max(1)
        ))
        .await?;
        self.write("isready").await?;
        self.wait_for(&["readyok"]).await?;

        self.write("ucinewgame").await?;
        self.write(&format!("position fen {}", query.fen)).await?;
        if let Some(ms) = query.movetime_ms {
            self.write(&format!("go movetime {}", ms.clamp(50, 30_000)))
                .await?;
        } else {
            self.write(&format!("go depth {}", query.depth.clamp(1, 30)))
                .await?;
        }

        let mut lines: BTreeMap<usize, EngineLine> = BTreeMap::new();
        let mut max_depth = 0u8;
        let mut buf = String::new();

        let best_move: Option<String> = loop {
            buf.clear();
            let n = self
                .stdout
                .read_line(&mut buf)
                .await
                .context("failed reading stockfish search output")?;
            if n == 0 {
                anyhow::bail!("stockfish exited during search");
            }
            let trimmed = buf.trim();
            if let Some(rest) = trimmed.strip_prefix("bestmove") {
                break rest.split_whitespace().next().map(str::to_string);
            }
            if !trimmed.starts_with("info ") || !trimmed.contains(" pv ") {
                continue;
            }
            if let Some(parsed) = parse_info(trimmed) {
                max_depth = max_depth.max(parsed.depth);
                let entry = lines.entry(parsed.multipv).or_insert_with(|| EngineLine {
                    multipv: parsed.multipv,
                    depth: 0,
                    score_cp: None,
                    score_mate: None,
                    pv: Vec::new(),
                });
                if parsed.depth >= entry.depth {
                    entry.depth = parsed.depth;
                    entry.score_cp = parsed.score_cp;
                    entry.score_mate = parsed.score_mate;
                    entry.pv = parsed.pv;
                }
            }
        };

        let lines: Vec<EngineLine> = lines.into_values().collect();
        Ok(EngineEval {
            best_move,
            depth: max_depth,
            lines,
        })
    }

    async fn shutdown(&mut self) {
        let _ = self.write("quit").await;
        let _ = self.child.kill().await;
    }
}

struct ParsedInfo {
    multipv: usize,
    depth: u8,
    score_cp: Option<i32>,
    score_mate: Option<i32>,
    pv: Vec<String>,
}

fn parse_info(line: &str) -> Option<ParsedInfo> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut multipv = 1usize;
    let mut depth = 0u8;
    let mut score_cp = None;
    let mut score_mate = None;
    let mut pv = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "multipv" => {
                if let Some(v) = tokens.get(i + 1).and_then(|v| v.parse::<usize>().ok()) {
                    multipv = v.max(1);
                }
                i += 2;
            }
            "depth" => {
                if let Some(v) = tokens.get(i + 1).and_then(|v| v.parse::<u8>().ok()) {
                    depth = v;
                }
                i += 2;
            }
            "score" => {
                match (tokens.get(i + 1), tokens.get(i + 2)) {
                    (Some(&"cp"), Some(v)) => {
                        score_cp = v.parse::<i32>().ok();
                        score_mate = None;
                    }
                    (Some(&"mate"), Some(v)) => {
                        score_mate = v.parse::<i32>().ok();
                        score_cp = None;
                    }
                    _ => {}
                }
                i += 3;
            }
            "pv" => {
                pv = tokens[i + 1..]
                    .iter()
                    .map(|s| s.to_string())
                    .filter(|s| {
                        s.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    })
                    .collect();
                break;
            }
            _ => i += 1,
        }
    }
    if pv.is_empty() && score_cp.is_none() && score_mate.is_none() {
        return None;
    }
    Some(ParsedInfo {
        multipv,
        depth,
        score_cp,
        score_mate,
        pv,
    })
}

async fn worker_loop(
    path: String,
    hash_mb: u32,
    query_timeout: Duration,
    rx: Receiver<Job>,
    metrics: Arc<Metrics>,
) {
    let mut proc: Option<EngineProcess> = None;

    while let Ok((query, reply)) = rx.recv().await {
        if proc.is_none() {
            match EngineProcess::spawn(&path, hash_mb).await {
                Ok(p) => proc = Some(p),
                Err(err) => {
                    metrics.stockfish_errors.inc();
                    let _ = reply.send(Err(format!("failed to start Stockfish: {err}")));
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            }
        }

        let Some(engine) = proc.as_mut() else {
            let _ = reply.send(Err("stockfish process unavailable".to_string()));
            continue;
        };

        match timeout(query_timeout, engine.evaluate(&query)).await {
            Ok(Ok(eval)) => {
                let _ = reply.send(Ok(eval));
            }
            Ok(Err(err)) => {
                metrics.stockfish_errors.inc();
                tracing::warn!(error = %err, "stockfish query failed; respawning worker");
                engine.shutdown().await;
                proc = None;
                let _ = reply.send(Err(format!("stockfish query failed: {err}")));
            }
            Err(_) => {
                metrics.stockfish_errors.inc();
                tracing::warn!(
                    timeout_ms = query_timeout.as_millis(),
                    "stockfish query timed out; killing worker"
                );
                engine.shutdown().await;
                proc = None;
                let _ = reply.send(Err("stockfish query timed out".to_string()));
            }
        }
    }

    if let Some(mut engine) = proc {
        engine.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multipv_info_line() {
        let line = "info depth 18 seldepth 24 multipv 2 score cp -37 nodes 100000 nps 900000 time 111 pv e2e4 e7e5 g1f3";
        let parsed = parse_info(line).expect("line should parse");
        assert_eq!(parsed.multipv, 2);
        assert_eq!(parsed.depth, 18);
        assert_eq!(parsed.score_cp, Some(-37));
        assert_eq!(parsed.pv, vec!["e2e4", "e7e5", "g1f3"]);
    }

    #[test]
    fn parses_mate_score() {
        let line = "info depth 12 multipv 1 score mate 3 pv h5f7 e8f7 d1f3";
        let parsed = parse_info(line).expect("line should parse");
        assert_eq!(parsed.score_mate, Some(3));
        assert!(parsed.score_cp.is_none());
    }
}
