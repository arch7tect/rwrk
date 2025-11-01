# Performance Comparison: rwrk vs wrk

## Results

rwrk (Rust) achieves **performance parity** with wrk (C) on I/O-bound HTTP benchmarking workloads.

| Tool | Throughput | Transfer/sec | Concurrency | Architecture |
|------|-----------|--------------|-------------|--------------|
| wrk | 7,039 req/sec | 11.45 MB/sec | 1024 connections (64 threads) | C + epoll/kqueue |
| **rwrk** | **6,939 req/sec** | 9.82 MB/sec | 1024 async workers | Rust + Tokio + Hyper |

**Performance difference: ~1.4% - essentially equal**

## Test Environment

- **Platform**: macOS, 16 cores
- **Target**: https://exposeme.org/
- **Duration**: 30 seconds
- **Concurrency**: 1024 operations

## Configuration

**rwrk**: 64x CPU cores = 1024 async workers (default)
```bash
./target/release/rwrk -u https://exposeme.org/ -t 30
```

**wrk**: 64 threads, 1024 connections
```bash
wrk -t64 -c1024 -d30s https://exposeme.org/
```

## Key Findings

1. **Async Rust matches C performance** for I/O-bound workloads
2. Both tools achieve ~7,000 req/sec at 1024 concurrent operations
3. rwrk provides memory safety and modern async/await without performance penalty
4. Transfer measurement differs: wrk counts headers+body, rwrk counts body only

## Implementation

- HTTP client: Hyper (low-level)
- Async runtime: Tokio
- Connection pooling: Matches worker count
- Metrics: Response body bytes
