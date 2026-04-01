# ARCHITECTURE

## Executive summary

This repository is a Rust Electrum server + Esplora HTTP backend adapted for DigiByte (`--network digibyte`) with the `new_index` storage model.

Core design:
- **Ingestion/indexing path:** pull blocks from bitcoind RPC or blk*.dat files, write canonical data into RocksDB (`txstore`, `history`, `cache`).
- **Query path:** compose chain index + live mempool into a unified `Query` API.
- **Serving path:** expose data through both Electrum JSON-RPC (`src/electrum/server.rs`) and REST (`src/rest.rs`).
- **Operational path:** config parsing, metrics export, periodic sync, signal-based reload, and optional discovery announcement.

## High-level architecture (ASCII)

```text
                     +---------------------------+
                     |      DigiByte Core        |
                     |      (digibyted RPC)      |
                     +------------+--------------+
                                  |
                        headers/blocks/tx/mempool
                                  |
                    +-------------v--------------+
                    |    daemon.rs (RPC client)  |
                    +-------------+--------------+
                                  |
                     +------------v-------------+
                     | new_index/fetch.rs       |
                     | (Bitcoind or blk*.dat)   |
                     +------------+-------------+
                                  |
                          block batches
                                  |
                     +------------v-------------------------------+
                     | new_index/schema.rs::Indexer              |
                     | - add(): txstore rows (T/C/O/B/X/M/D)     |
                     | - index(): history rows (H/S/a, etc.)     |
                     +------------+-------------------------------+
                                  |
                 +----------------+----------------+
                 |                                 |
        +--------v---------+             +---------v--------+
        | RocksDB txstore  |             | RocksDB history  |
        +------------------+             +------------------+
                 |                                 |
                 +----------------+----------------+
                                  |
                           +------v------+
                           | cache DB    |
                           | A/U entries |
                           +------+------+
                                  |
                     +------------v-------------+
                     | new_index/query.rs::Query|
                     | + mempool.rs::Mempool    |
                     +------------+-------------+
                                  |
                  +---------------+----------------+
                  |                                |
        +---------v---------+            +---------v----------+
        | electrum/server.rs|            | rest.rs (HTTP API) |
        +-------------------+            +--------------------+
```

## Directory structure

- `src/bin/` – executables (`electrs`, utility binaries)
- `src/new_index/` – indexing, RocksDB access, fetchers, query composition, mempool tracking
- `src/electrum/` – Electrum protocol server/client/discovery
- `src/rest.rs` – Esplora-style HTTP API endpoints
- `src/util/` – block/tx/script helpers, hashing, bincode, merkle, fees
- `src/config.rs` – CLI/env config and network/runtime options
- `src/daemon.rs` – JSON-RPC client to node backend
- `src/metrics.rs` – Prometheus metrics endpoint and process exporter
- `tests/` – integration tests for REST/Electrum
- `doc/` – usage and DB schema docs

## Key components

- **`Config` (`src/config.rs`)**
  - Central runtime configuration (network, RPC, DB paths, REST/Electrum ports, limits, feature flags like light mode / address search).

- **`Daemon` (`src/daemon.rs`)**
  - Persistent/reconnecting RPC transport for block headers, blocks, tx lookups, mempool, fee estimates, broadcasting.

- **`Store` + `Indexer` (`src/new_index/schema.rs`)**
  - `Store` opens/owns the three RocksDB databases.
  - `Indexer::update()` drives sync cycles and writes rows for tx/block/history/edge indexes.

- **`ChainQuery` (`src/new_index/schema.rs`)**
  - Read-side API over indexed chain state (history, utxo, block metadata, tx lookups, confirmations, merkle proofs).

- **`Mempool` (`src/new_index/mempool.rs`)**
  - Live unconfirmed state (history deltas, spending edges, fee stats, recent tx list).

- **`Query` (`src/new_index/query.rs`)**
  - Facade that merges `ChainQuery` + `Mempool` and provides API-oriented operations.

- **REST server (`src/rest.rs`)**
  - Esplora endpoints for blocks, txs, address/scripthash history, utxo, mempool, fee estimates, broadcast.
  - Includes DigiByte-specific address/scripthash handling (`dgb1`/script conversion path).

- **Electrum server (`src/electrum/server.rs`)**
  - Handles Electrum JSON-RPC methods and subscriptions.

## Data flow

1. **Startup**
   - Parse config, init metrics, open DBs, load headers/tip, connect daemon.
2. **Sync/index loop**
   - Fetch new headers → fetch matching blocks → `add` phase (txstore) → `index` phase (history/edges/address index) → persist new synced tip.
3. **Mempool refresh**
   - Track additions/removals and maintain in-memory overlays.
4. **Serving queries**
   - REST/Electrum request hits `Query`, which composes chain-confirmed state + mempool state and returns normalized responses.

## Config & deployment

- Main manifest/tooling: `Cargo.toml`, `rust-toolchain.toml`.
- Runtime usage docs: `README.md`, `doc/usage.md`.
- Schema documentation: `doc/schema.md`.
- Launch script: `scripts/run.sh`.
- CI baseline: `.travis.yml`.

Important runtime knobs:
- network selection (`digibyte`, regtest, etc.)
- RPC auth and daemon paths
- `--lightmode` (smaller disk, more RPC lookups)
- REST bind/socket + CORS
- address search index (`--address-search`)
- UTXO / electrum history limits

## Design patterns and implementation choices

- **Two-phase indexing** (`add` then `index`) to ensure spending history can resolve prevouts efficiently.
- **Prefix-key RocksDB schema** for scan-friendly historical queries.
- **Read-through caches** (`A`/`U` cache DB rows) to accelerate stats/utxo for hot scripts.
- **Parallelism** via rayon for CPU-heavy serialization/indexing work.
- **Separation of concerns**
  - daemon transport
  - indexing storage mechanics
  - query composition
  - protocol adapters (REST/Electrum)
- **Feature-gated multi-chain support** (`liquid` modules and paths isolated via cfg flags).

## Operational notes

- Initial sync can source data from blk files then fall back to RPC.
- Metrics exporter includes process stats and request/index histograms.
- Signal handling supports controlled sync/reload behavior from runtime events.
