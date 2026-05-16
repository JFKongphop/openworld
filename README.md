# OpenWorld

**Autonomous agentic travel planning and reservation system powered by [0G Compute](https://0g.ai) + [0G Storage](https://0g.ai), with on-chain journey proofs via ERC-7857.**

Write a `trip.md` policy file. A pipeline of AI agents handles everything — flights, hotels, transport, budget enforcement, failure recovery — and mints a verifiable NFT artifact on the 0G Mainnet when done.

---

## How It Works

```
trip.md  →  Orchestrator  →  [PlannerAgent → SearchAgent → VaultAgent
                               → ReservationAgent → RecoveryAgent
                               → VaultAgent → ArtifactAgent]
                           →  ERC-7857 Journey NFT on 0G Mainnet
```

### Agent Pipeline

| Agent | Role |
|---|---|
| **PlannerAgent** | Generates a structured day-by-day itinerary using 0G Compute (LLM) |
| **SearchAgent** | Researches and prices flights, hotels, and transport via 0G Compute |
| **VaultAgent** | Enforces budget caps and approves/rejects each transaction |
| **ReservationAgent** | Executes bookings via OpenClaw browser automation (or simulation) |
| **RecoveryAgent** | Detects failed bookings and replans alternatives using 0G Compute |
| **ArtifactAgent** | Hashes all execution data, uploads to 0G Storage, mints ERC-7857 NFT |

---

## Repository Structure

```
openworld/
├── backend/          # Rust orchestration engine & API server
│   ├── src/
│   │   ├── agents/   # PlannerAgent, SearchAgent, VaultAgent, ReservationAgent,
│   │   │             #   RecoveryAgent, ArtifactAgent
│   │   ├── api.rs    # Axum HTTP + WebSocket API server
│   │   ├── erc7857.rs        # On-chain minting via ethers-rs
│   │   ├── og_compute.rs     # 0G Compute (LLM) client
│   │   ├── og_storage.rs     # 0G Storage client (via 0g-cli)
│   │   ├── orchestrator.rs   # Session state machine & pipeline runner
│   │   ├── report.rs         # Markdown travel report generator
│   │   └── travel_spec.rs    # trip.md YAML parser
│   ├── examples/trip.md      # Example trip policy
│   └── reports/              # Generated Markdown reports
├── contract/         # Solidity smart contract
│   └── src/
│       └── OpenWorldJourney.sol  # ERC-7857 Intelligent NFT
├── frontend/         # React + TypeScript UI
└── README.md
```

---

## The `trip.md` Format

Define your travel policy in YAML:

```yaml
trip:
  owner: "0xYourWalletAddress"   # EVM wallet that receives the journey NFT
  origin: BKK                    # Departure city / IATA code
  destination: TYO               # IATA city code (single-city) or country name
  departure_date: "2026-06-10"
  return_date:    "2026-06-15"
  budget_max: "1500 USD"

flight:
  max_stops: 1
  avoid_red_eye: true
  preferred_airlines: [ANA, JAL]

hotel:
  min_rating: 4.0
  max_price_per_night: "100 USD"
  near_station: true

transport:
  prefer_train: true
  avoid_overnight_bus: true

automation:
  auto_reserve: true
  retry_on_failure: true
  allow_replanning: true

vault:
  auto_payment: true
  max_single_transaction: "500 USD"
```

Set `destination` to an **IATA city code** (e.g. `TYO`, `OSA`, `SIN`) for single-city mode, or a **country/region name** (e.g. `Japan`, `France`) for a multi-city itinerary.

---

## Setup

### Prerequisites

- Rust (`cargo`)
- Node.js + pnpm
- [Foundry](https://getfoundry.sh) (`forge`)
- [`0g-cli`](https://github.com/0glabs/0g-storage-client) binary in `backend/`

### 1. Backend environment

Copy and fill in `backend/.env`:

```bash
cp backend/.env.example backend/.env
```

| Variable | Description |
|---|---|
| `OG_COMPUTE_ENDPOINT` | 0G Compute chat completions URL |
| `OG_COMPUTE_MODEL` | LLM model (e.g. `qwen/qwen-2.5-7b-instruct`) |
| `OG_COMPUTE_API_KEY` | Bearer token for 0G Compute |
| `OG_INDEXER_RPC` | 0G Storage indexer RPC |
| `OG_STORAGE_STREAM_ID` | Stream ID for storage uploads |
| `OG_CLI_PATH` | Path to `0g-cli` binary |
| `FIRECRAWL_API_KEY` | Firecrawl key for SearchAgent web scraping |
| `OPENCLAW_ENDPOINT` | OpenClaw URL for browser automation (leave blank for simulation) |
| `OG_RPC_URL` | 0G Mainnet RPC (`https://evmrpc.0g.ai`) |
| `JOURNEY_CONTRACT_ADDRESS` | Deployed `OpenWorldJourney` contract address |
| `OPERATOR_PRIVATE_KEY` | Agent operator wallet private key |

### 2. Build & run the backend

```bash
# Run a trip end-to-end (CLI)
cd backend
cargo run --bin travel -- examples/trip.md

# Start the API server (for the frontend)
cargo run --bin api

# Generate a Markdown report from a completed session
cargo run --bin report -- <session_id>
```

### 3. Run the frontend

```bash
cd frontend
pnpm install
pnpm dev
```

---

## API Reference

The API server listens on `http://localhost:3000` by default.

| Method | Path | Description |
|---|---|---|
| `POST` | `/sessions` | Create a session from `travel_md` YAML |
| `POST` | `/sessions/:id/start` | Start the orchestration pipeline |
| `GET` | `/sessions/:id` | Full session state (policy, itinerary, bookings, artifact, logs) |
| `GET` | `/sessions/:id/logs` | Activity log entries |
| `GET` | `/sessions/:id/artifact` | ERC-7857 journey artifact JSON |
| `GET` | `/ws/:id` | WebSocket — streams `ActivityLog` as JSON lines in real time |
| `GET` | `/health` | Health check |

---

## Smart Contract

`OpenWorldJourney` is an ERC-7857 Intelligent NFT deployed on 0G Mainnet (chain ID `16661`).

Each token represents one completed journey execution and stores two `IntelligentData` collections:

1. **JSON Context Memory** — session policy, AI itinerary, booking results, vault ledger, and execution logs (0G Storage root hash)
2. **Report Root** — 0G Storage root hash of the generated Markdown travel report

**Deployed contract:** [`0x770f6107934224882ce4919934eE5B2BfF7783aE`](https://chainscan.0g.ai/address/0x770f6107934224882ce4919934eE5B2BfF7783aE) on 0G Mainnet (chain ID `16661`)

### Deploy

```bash
cd contract
forge script script/DeployOpenWorldJourney.s.sol \
  --rpc-url https://evmrpc.0g.ai \
  --private-key $OPERATOR_PRIVATE_KEY \
  --broadcast \
  --verify
```

After deploying, set `JOURNEY_CONTRACT_ADDRESS` in `backend/.env`.

---

## On-Chain Proof

After a trip completes, the `ArtifactAgent`:

1. Hashes all confirmed booking references
2. Hashes the full orchestration log
3. Uploads a `StoredArtifact` JSON blob to 0G Storage
4. Calls `mintAndRecord()` on the contract — producing an immutable, verifiable on-chain record of the autonomous execution

The journey NFT is transferred to the `owner` address specified in `trip.md`.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Agent reasoning | 0G Compute (OpenAI-compatible LLM API) |
| Persistent memory | 0G Storage + `0g-cli` |
| On-chain record | ERC-7857 NFT on 0G Mainnet |
| Backend | Rust, Axum, ethers-rs, tokio |
| Frontend | React, TypeScript, Vite, Tailwind CSS |
| Smart contract | Solidity 0.8.20, Foundry, OpenZeppelin |

---

## License

MIT
