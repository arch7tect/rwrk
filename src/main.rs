use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use clap::Parser;
use reqwest::Client;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Parser, Debug)]
#[command(name = "rwrk")]
#[command(about = "Process millions of tasks with timeout", long_about = None)]
struct Config {
    /// Base URL (use {id} as placeholder)
    #[arg(short = 'u', long)]
    url: String,

    #[arg(short = 'n', long, default_value = "5000000")]
    total_tasks: usize,

    #[arg(short = 't', long, default_value = "10")]
    timeout_secs: u64,

    #[arg(short = 'w', long)]
    worker_count: Option<usize>,

    #[arg(short = 'l', long, default_value = "info")]
    log_level: String,
}

#[derive(Default)]
struct WorkerStats {
    completed: u64,
    successful: u64,
    bytes: u64,
}

#[tokio::main]
async fn main() {
    let config = Config::parse();

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&config.log_level)))
        .init();

    let worker_count = config.worker_count.unwrap_or_else(|| num_cpus::get() * 4);

    let start = Instant::now();
    info!("Starting high-performance HTTP task processor");
    info!(url = %config.url, "Target URL");
    info!(
        total_tasks = config.total_tasks,
        workers = worker_count,
        timeout_secs = config.timeout_secs,
        "Configuration"
    );

    let client = Arc::new(
        Client::builder()
            .pool_max_idle_per_host(worker_count)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build HTTP client")
    );
    let cancel_token = CancellationToken::new();

    let cancel_token_timeout = cancel_token.clone();
    let timeout_secs = config.timeout_secs;
    tokio::spawn(async move {
        sleep(Duration::from_secs(timeout_secs)).await;
        warn!("Timeout reached, cancelling all tasks");
        cancel_token_timeout.cancel();
    });

    let requests_per_worker = config.total_tasks / worker_count;
    let remainder = config.total_tasks % worker_count;

    let has_placeholder = config.url.contains("{id}");
    let static_url = if !has_placeholder {
        Some(Arc::new(config.url.clone()))
    } else {
        None
    };

    let mut workers = Vec::new();

    for worker_id in 0..worker_count {
        let client = client.clone();
        let cancel_token = cancel_token.clone();
        let base_url = if let Some(url) = &static_url {
            url.clone()
        } else {
            Arc::new(config.url.clone())
        };

        let my_requests = requests_per_worker + if worker_id < remainder { 1 } else { 0 };
        let start_id = (worker_id * requests_per_worker + worker_id.min(remainder)) as u64;

        let worker = tokio::spawn(async move {
            let mut stats = WorkerStats::default();

            for offset in 0..my_requests {
                if cancel_token.is_cancelled() {
                    break;
                }

                let request = if has_placeholder {
                    let id = start_id + offset as u64;
                    let url = base_url.replace("{id}", &id.to_string());
                    client.get(&url)
                } else {
                    client.get(base_url.as_ref())
                };

                match request.send().await {
                    Ok(response) => {
                        let success = response.status().is_success();
                        match response.bytes().await {
                            Ok(body) => {
                                stats.completed += 1;
                                if success {
                                    stats.successful += 1;
                                }
                                stats.bytes += body.len() as u64;
                            }
                            Err(_) => {
                                stats.completed += 1;
                            }
                        }
                    }
                    Err(_) => {
                        stats.completed += 1;
                    }
                }
            }

            stats
        });

        workers.push(worker);
    }

    let mut total_completed = 0u64;
    let mut total_successful = 0u64;
    let mut total_bytes = 0u64;

    for worker in workers {
        if let Ok(stats) = worker.await {
            total_completed += stats.completed;
            total_successful += stats.successful;
            total_bytes += stats.bytes;
        }
    }

    cancel_token.cancel();

    let elapsed = start.elapsed();

    let success_rate = if total_completed > 0 {
        (total_successful as f64 / total_completed as f64) * 100.0
    } else {
        0.0
    };

    let mb = total_bytes as f64 / 1_048_576.0;
    let bytes_display = if mb >= 1.0 {
        format!("{:.2} MB", mb)
    } else {
        format!("{:.2} KB", total_bytes as f64 / 1024.0)
    };

    let throughput = total_completed as f64 / elapsed.as_secs_f64();
    let bandwidth_mb = (total_bytes as f64 / 1_048_576.0) / elapsed.as_secs_f64();

    info!("=== Statistics ===");
    info!(
        total_time_secs = %format!("{:.2}", elapsed.as_secs_f64()),
        tasks_completed = total_completed,
        "Execution summary"
    );
    info!(
        successful_requests = total_successful,
        success_rate = %format!("{:.1}%", success_rate),
        bytes_transferred = %bytes_display,
        "Request results"
    );
    info!(
        throughput = %format!("{:.0} req/sec", throughput),
        bandwidth = %format!("{:.2} MB/sec", bandwidth_mb),
        "Performance"
    );
}
