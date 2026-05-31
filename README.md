# Solana Three-Tier Token Indexer (TokenIndexer Backend)

A high-performance, event-driven Solana token indexing pipeline built in Rust using **Actix-web**, **Tokio**, and **SQLx (PostgreSQL)**. It implements a decoupled, three-tier asynchronous architecture designed to discover newly launched tokens, index their transaction history, isolate highly profitable early buyers, and track those identified whale wallets in real-time.

---

## 🏛️ Architecture & Three-Tier Pipeline

The platform splits indexer duties into three independent, concurrent runner modules running on a Tokio async scheduler, communicating through a PostgreSQL persistent store and a high-throughput, in-memory event broadcast channel.

```mermaid
flowchart TD
    subgraph Solana Blockchain
        Solana[Solana RPC Nodes]
    end

    subgraph Three-Tier Indexer Nodes
        Scout[Node 1: The Scout]
        Hunter[Node 2: The Hunter]
        Shadow[Node 3: The Shadow]
    end

    subgraph Storage & Event Core
        DB[(PostgreSQL Store)]
        Queue[InMemory Message Queue]
        WS[WebSocket Broadcast Channel]
    end

    subgraph Consumer Clients
        API[Actix-web REST API]
        Client[React Frontend Vercel]
    end

    %% Flow mappings
    Solana -->|Poll Blocks| Scout
    Solana -->|Trace History| Hunter
    Solana -->|Monitor Wallets| Shadow

    Scout -->|Inserts Discovered Tokens| DB
    Hunter -->|Extracts Early Transactions| DB
    Shadow -->|Generates Alerts| DB

    Shadow -->|Publishes Events| Queue
    Queue -->|Pipes Alerts| WS
    WS -.->|Real-time alerts| Client
    DB <--> API
    API <--> Client
```

---

## 🚀 The Three Indexers: Roles & Logic

### 1. The Scout (Token Discovery Node)
* **Goal**: Real-time detection of new token launches.
* **Mechanism**: 
  - Periodically polls the latest Solana block slots (configured by `polling_interval_secs` under `[scout]`).
  - Scans block transaction arrays looking for signatures interacting with target launchpad contract programs:
    - **Pump.fun**: `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
    - **Raydium**: `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`
  - Utilizes standard JSON Pointer paths aligned with Solana's transaction serialization schemas to extract the newly created token mint addresses.
  - Inserts the new token metadata into the `tokens` table with `last_indexed_at = NULL` (represented as a **Pending** status on the client application).

### 2. The Hunter (Intel Analyzer Node)
* **Goal**: Deep audit of early token transactions to flag profitable snipers and whales.
* **Mechanism**:
  - Monitors mature and unanalyzed tokens (e.g. tokens discovered at least `token_maturity_minutes` ago).
  - Fetches the transaction signatures history for the target token up to a configurable depth limit (`max_signatures_per_token`).
  - Audits pre- and post-token balances (`preTokenBalances`/`postTokenBalances`) to extract absolute purchase quantities and buyer wallet addresses.
  - Classifies early buyer transactions if the transaction slot occurs within the initial launch slot window (`early_buyer_window_secs`).
  - Inserts all audited transactions into the `token_transactions` database table and marks the token's `last_indexed_at` column as indexed.

### 3. The Shadow (Whale Tracking Node)
* **Goal**: Live monitoring of proven whale wallets.
* **Mechanism**:
  - Loads the complete list of qualified whale wallets from the database (filtering up to `max_concurrent_wallets`).
  - Spawns concurrent, low-latency background polling tasks to fetch recent transaction signatures for each whale address.
  - When a transaction signature indicates a new token buy swap, it instantiates a `WhaleAlert` record.
  - Persists the alert to `whale_alerts` in the PostgreSQL database and immediately publishes a `WhalePurchase` event to the `InMemoryQueue`.

---

## 💎 Whale Classification Criteria

Wallets are qualified as **Whale Wallets** under `WhaleWalletRepo` by satisfying configurable trading performance and profitability benchmarks:

* **Win Rate (`whale_win_rate_threshold`)**:
  - Default: **70%** (`0.70`). The proportion of profitable trades out of the total trades analyzed for the wallet.
* **ROI Threshold (`whale_roi_threshold`)**:
  - Default: **3.0x** (300% ROI). The average profit multiplier returned across all trades.

### Profiles:
1. **Established Sniper (`WalletType::EstablishedSniper`)**: High frequency of purchases in the initial blocks of token deployments.
2. **Consistent Trader (`WalletType::ConsistentTrader`)**: Sustains high win rates over prolonged intervals.
3. **High ROI Wallet (`WalletType::HighRoiWallet`)**: Captures exceptional percentage multipliers.

---

## ⚡ Tech Stack & Key Crates

* **Actix-web (4.9)**: Drives the HTTP server hosting REST endpoints, rate-limiting middleware, and high-performance WebSocket connections.
* **Tokio (1.41)**: Asynchronous runtime executing multi-threaded execution pools, task selection, and channel coordination.
* **SQLx (0.8)**: Handles asynchronous, compile-time verified queries and connection pooling for PostgreSQL.
* **config (0.14) & dotenvy (0.15)**: Manages layered environment parsing across system properties, `.env` files, and `config.toml`.
* **tracing**: Emits highly structured, low-overhead JSON logs to standard output.

---

## ⚙️ Configuration & Environment

All indexer configurations can be customized via `config.toml` or overridden in production environments using variables prefixed with `APP_` and separated by double underscores `__`.

### Key Environment Mappings
```bash
# Database connection pool url
APP_DATABASE__URL=postgres://user:password@host:5432/dbname

# Target Solana RPC endpoints list (must be a JSON array string)
APP_RPC__ENDPOINTS=["https://api.mainnet-beta.solana.com"]

# Network interface and port Actix binds to
APP_API__HOST=0.0.0.0
APP_API__PORT=8080
```

---

## 🛠️ Getting Started Locally

### 1. Prerequisites
- Rust compiler and Cargo toolchain (`rustup`)
- Running PostgreSQL database instance

### 2. Compilation & Build
Compile and optimize release builds of the main indexer binary:
```bash
cargo build --release
```

### 3. Run Tests
Verify indexer parsing correctness, mock queues, and properties using proptests:
```bash
cargo test
```

### 4. Running the Pipeline
Run the database migrations and spin up all three indexer nodes along with the Actix-web server:
```bash
cargo run --release --bin solana-indexer
```
