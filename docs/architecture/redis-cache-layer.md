# In-memory Cache Layer with Redis

## Architecture
This document details the Redis cache layer architecture. The system uses a centralized Redis cluster for high-speed in-memory caching.
- **TTL**: Configurable TTL per key to prevent memory exhaustion and serve fresh data.
- **Performance target**: < 100ms P99 latency.
- **Availability target**: 99.99% uptime with Redis cluster replication and automatic failover.
- **Security**: Access controlled via TLS and Redis AUTH.

## Deployment Strategy
We use a blue-green deployment strategy for upgrading the cache layer. Canary analysis is used for observing the cache hit/miss ratio before rolling out fully.
