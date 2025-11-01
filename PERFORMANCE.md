# Performance Comparison: rwrk vs wrk

## Results

rwrk (Rust) outperforms wrk (C) by **19.8%** on I/O-bound HTTP benchmarking workloads.

| Tool | Throughput | Transfer/sec | Concurrency | Architecture |
|------|-----------|--------------|-------------|--------------|
| **rwrk** | **7,077 req/sec** | 10.01 MB/sec | 1024 async workers | Rust + Tokio + Hyper |
| wrk | 5,905 req/sec | 9.61 MB/sec | 64 OS threads | C + epoll/kqueue |

## Test Environment

- **Platform**: macOS, 16 cores
- **Target**: https://exposeme.org/
- **Duration**: 30 seconds
- **Connections**: 512

## Optimal Configuration

**rwrk**: 64x CPU cores = 1024 async workers
```bash
./target/release/rwrk -u https://exposeme.org/ -t 30
```

**wrk**: 4x CPU cores = 64 OS threads
```bash
wrk -t64 -c512 -d30s https://exposeme.org/
```

## Key Findings

1. **Async tasks scale better than OS threads** for I/O-bound workloads
2. **rwrk optimal at 1024 workers** vs wrk optimal at 64 threads
3. Async runtime enables higher concurrency without thread context-switching overhead
4. Rust achieves C-level performance with memory and thread safety guarantees

## Implementation

- HTTP client: Hyper (low-level)
- Async runtime: Tokio
- Connection pooling: Matches worker count
- Metrics: Response body bytes only
