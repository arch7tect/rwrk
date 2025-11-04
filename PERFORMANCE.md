# Performance Comparison: rwrk vs wrk

## Results

rwrk (Rust) **matches wrk performance** on controlled benchmarks against nginx.

| Tool | Median (req/sec) | Average (req/sec) | Peak (req/sec) | Std Dev | Concurrency | Architecture |
|------|------------------|-------------------|----------------|---------|-------------|--------------|
| **rwrk** | **46,711** | 46,871 | 47,433 | ±385 | 768 async workers, 1536 pool | Rust + Tokio + Hyper |
| wrk | 46,638 | 46,744 | 47,580 | ±521 | 704 connections (64 threads) | C + epoll/kqueue |

**Performance difference: 0.16% (median) - essentially identical performance**

## Test Environment

- **Platform**: macOS, 16 cores
- **Target**: nginx:alpine (Docker, localhost)
- **Duration**: 30 seconds

### Setup nginx for testing:
```bash
docker run -d --name nginx-bench -p 9090:80 nginx:alpine
```

## Optimal Configuration

**rwrk**: 48x CPU cores = 768 async workers, 2x connection pool (default)
```bash
./target/release/rwrk -u http://127.0.0.1:9090/ -t 30
# Explicitly: -w 768 -c 1536
```

**wrk**: 64 threads, 704 connections
```bash
wrk -t64 -c704 -d30s http://127.0.0.1:9090/
```

## Key Findings

1. **Async Rust matches C performance** (median 46,711 vs 46,638 req/sec - 0.16% difference)
2. rwrk is more consistent (std dev ±385 vs wrk's ±521)
3. **Zero errors across all 20 runs** (10 runs each tool, ~1.4M requests total per run)
4. Optimal configuration: 768 workers (48x CPU cores) with 2x connection pool on 16-core system
5. Connection pooling: Setting pool size to 2x worker count provides best results

## Implementation

- HTTP client: Hyper (low-level, direct protocol implementation)
- Async runtime: Tokio
- Connection pooling: Configurable idle connection pool per host
- Cancellation: tokio::select! for responsive timeout handling
- Metrics: Full response body bytes counted frame-by-frame

## Configuration Parameters

- `-w, --worker-count`: Number of concurrent workers (default: num_cpus * 48)
- `-c, --pool-max-idle-per-host`: Max idle connections per host (default: 2x worker count)
- `-i, --pool-idle-timeout`: Idle connection timeout in seconds (default: 90)

## Benchmark Methodology

All benchmarks run against nginx:alpine in Docker on localhost to eliminate network variability.

**Control Series Testing:**
- Two independent test series conducted
- Each series: 5 runs of rwrk, 5 runs of wrk
- Total: 10 runs per tool
- Results averaged across all 10 runs
- Standard deviation calculated to measure consistency

**Series 1:** rwrk avg 46,827 | wrk avg 46,562
**Series 2:** rwrk avg 46,914 | wrk avg 46,926
