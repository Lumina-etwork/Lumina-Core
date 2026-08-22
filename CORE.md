# Lumina-Core Guide

Welcome to the **Lumina-Core** core documentation guide. This guide consolidates all necessary documentation, including README information, API references, and architecture details into a single source of truth.

## Overview
Lumina-Core is a Rust library providing core modules such as identity registry, connection pool shard management, and attestation.

## Installation & Setup
To use this library in your Rust project, add it to your `Cargo.toml`:

```toml
[dependencies]
lumina-core = { path = "path/to/Lumina-Core" }
```

## API Reference (Exports)

The library exposes the following root modules:

### `core`
The `core` module includes lower-level mechanisms and pool shard managers.
- `core::pool::ShardManager`: Manages connection pool shards across tenants, using a free-list to prevent shard leaks under concurrent release/reclaim.
- `core::pool::ShardState`: Represents the state and unique identifiers for shards.

### `pool`
The `pool` module implements congestion control mechanisms.
- `pool::congestion`: Network and task congestion handlers.

### `identity`
The `identity` module handles identity registries.
- `identity::registry`: Identity management and registry structures.

## Contributing
See [CONTRIBUTING.md](./CONTRIBUTING.md) for details on how to contribute.
