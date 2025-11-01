use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use clap::Parser;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use hyper::Request;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use hyper_rustls::HttpsConnector;
use http_body_util::{BodyExt, Empty};
use bytes::Bytes;

type HttpsClient = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

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

    let worker_count = config.worker_count.unwrap_or_else(|| num_cpus::get() * 56);

    let start = Instant::now();
    info!("Running {}s test @ {}", config.timeout_secs, config.url);
    info!("  {} workers and {} max tasks", worker_count, config.total_tasks);

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .expect("Failed to load native roots")
        .https_only()
        .enable_http1()
        .build();

    let client: HttpsClient = Client::builder(TokioExecutor::new())
        .pool_max_idle_per_host(worker_count)
        .pool_idle_timeout(Duration::from_secs(90))
        .build(https);

    let client = Arc::new(client);
    let cancel_token = CancellationToken::new();

    let cancel_token_timeout = cancel_token.clone();
    let timeout_secs = config.timeout_secs;
    tokio::spawn(async move {
        sleep(Duration::from_secs(timeout_secs)).await;
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

                let url = if has_placeholder {
                    let id = start_id + offset as u64;
                    base_url.replace("{id}", &id.to_string())
                } else {
                    base_url.as_ref().clone()
                };

                let req = match Request::builder()
                    .uri(&url)
                    .body(Empty::<Bytes>::new())
                {
                    Ok(r) => r,
                    Err(_) => {
                        stats.completed += 1;
                        continue;
                    }
                };

                match client.request(req).await {
                    Ok(response) => {
                        let success = response.status().is_success();
                        match response.into_body().collect().await {
                            Ok(body) => {
                                stats.completed += 1;
                                if success {
                                    stats.successful += 1;
                                }
                                stats.bytes += body.to_bytes().len() as u64;
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

    let throughput = total_completed as f64 / elapsed.as_secs_f64();
    let bandwidth_mb = (total_bytes as f64 / 1_048_576.0) / elapsed.as_secs_f64();
    let mb = total_bytes as f64 / 1_048_576.0;
    let errors = total_completed - total_successful;

    info!("  {} requests in {:.2}s, {:.2}MB read", total_completed, elapsed.as_secs_f64(), mb);
    if errors > 0 {
        info!("  Non-2xx responses: {}", errors);
    }
    info!("Requests/sec:      {:.2}", throughput);
    info!("Transfer/sec:      {:.2}MB", bandwidth_mb);
    if total_completed < config.total_tasks as u64 {
        info!("Completed:         {}/{} tasks", total_completed, config.total_tasks);
    }
}
