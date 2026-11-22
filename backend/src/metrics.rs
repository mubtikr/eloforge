use prometheus::{
    Encoder, Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry, TextEncoder,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,
    pub analysis_jobs: IntCounter,
    pub analysis_failures: IntCounter,
    pub analysis_latency: Histogram,
    pub stockfish_queue_depth: IntGauge,
    pub stockfish_workers: IntGauge,
    pub stockfish_errors: IntCounter,
    pub ai_requests: IntCounter,
    pub ai_errors: IntCounter,
    pub ai_tokens: IntCounter,
    pub ai_latency: Histogram,
    pub sync_runs: IntCounter,
    pub sync_games: IntCounter,
    pub sync_errors: IntCounter,
    pub sse_connections: IntGauge,
    pub puzzles_created: IntCounter,
    pub active_websockets: Gauge,
}

impl Metrics {
    pub fn new() -> anyhow::Result<Arc<Self>> {
        let registry = Registry::new();

        let analysis_jobs = IntCounter::with_opts(Opts::new(
            "eloforge_analysis_jobs_total",
            "Total game analysis jobs processed",
        ))?;
        let analysis_failures = IntCounter::with_opts(Opts::new(
            "eloforge_analysis_failures_total",
            "Total failed game analysis jobs",
        ))?;
        let analysis_latency = Histogram::with_opts(
            HistogramOpts::new(
                "eloforge_analysis_latency_seconds",
                "Game analysis latency in seconds",
            )
            .buckets(vec![1.0, 2.5, 5.0, 10.0, 20.0, 40.0, 80.0, 160.0]),
        )?;
        let stockfish_queue_depth = IntGauge::with_opts(Opts::new(
            "eloforge_stockfish_queue_depth",
            "Queued Stockfish queries",
        ))?;
        let stockfish_workers = IntGauge::with_opts(Opts::new(
            "eloforge_stockfish_workers",
            "Configured Stockfish workers",
        ))?;
        let stockfish_errors = IntCounter::with_opts(Opts::new(
            "eloforge_stockfish_errors_total",
            "Stockfish query errors",
        ))?;
        let ai_requests = IntCounter::with_opts(Opts::new(
            "eloforge_ai_requests_total",
            "OpenCode Go requests",
        ))?;
        let ai_errors = IntCounter::with_opts(Opts::new(
            "eloforge_ai_errors_total",
            "OpenCode Go request failures",
        ))?;
        let ai_tokens = IntCounter::with_opts(Opts::new(
            "eloforge_ai_tokens_total",
            "OpenCode Go tokens consumed (reported or estimated)",
        ))?;
        let ai_latency = Histogram::with_opts(
            HistogramOpts::new(
                "eloforge_ai_latency_seconds",
                "OpenCode Go request latency in seconds",
            )
            .buckets(vec![0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0]),
        )?;
        let sync_runs =
            IntCounter::with_opts(Opts::new("eloforge_sync_runs_total", "Chess.com sync runs"))?;
        let sync_games = IntCounter::with_opts(Opts::new(
            "eloforge_sync_games_total",
            "Chess.com games imported",
        ))?;
        let sync_errors = IntCounter::with_opts(Opts::new(
            "eloforge_sync_errors_total",
            "Chess.com sync errors",
        ))?;
        let sse_connections = IntGauge::with_opts(Opts::new(
            "eloforge_sse_connections",
            "Open SSE connections",
        ))?;
        let puzzles_created = IntCounter::with_opts(Opts::new(
            "eloforge_puzzles_created_total",
            "Puzzles extracted from games",
        ))?;
        let active_websockets = Gauge::with_opts(Opts::new(
            "eloforge_active_websockets",
            "Active streaming connections",
        ))?;

        registry.register(Box::new(analysis_jobs.clone()))?;
        registry.register(Box::new(analysis_failures.clone()))?;
        registry.register(Box::new(analysis_latency.clone()))?;
        registry.register(Box::new(stockfish_queue_depth.clone()))?;
        registry.register(Box::new(stockfish_workers.clone()))?;
        registry.register(Box::new(stockfish_errors.clone()))?;
        registry.register(Box::new(ai_requests.clone()))?;
        registry.register(Box::new(ai_errors.clone()))?;
        registry.register(Box::new(ai_tokens.clone()))?;
        registry.register(Box::new(ai_latency.clone()))?;
        registry.register(Box::new(sync_runs.clone()))?;
        registry.register(Box::new(sync_games.clone()))?;
        registry.register(Box::new(sync_errors.clone()))?;
        registry.register(Box::new(sse_connections.clone()))?;
        registry.register(Box::new(puzzles_created.clone()))?;
        registry.register(Box::new(active_websockets.clone()))?;

        Ok(Arc::new(Self {
            registry,
            analysis_jobs,
            analysis_failures,
            analysis_latency,
            stockfish_queue_depth,
            stockfish_workers,
            stockfish_errors,
            ai_requests,
            ai_errors,
            ai_tokens,
            ai_latency,
            sync_runs,
            sync_games,
            sync_errors,
            sse_connections,
            puzzles_created,
            active_websockets,
        }))
    }

    pub fn render(&self) -> anyhow::Result<(String, Vec<u8>)> {
        let encoder = TextEncoder::new();
        let families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&families, &mut buffer)?;
        Ok((encoder.format_type().to_string(), buffer))
    }
}
