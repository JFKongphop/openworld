# OpenWorld — Autonomous AI Travel Operations

> **8 AI agents. Zero manual steps. Real flights, real hotels, real decisions.**

OpenWorld is a fully autonomous travel orchestration system powered by **Qwen AI (Alibaba Cloud)**. Write a `trip.md` policy file — a pipeline of specialized AI agents handles planning, search, budget enforcement, reservations, failure recovery, and final report generation — end to end, without human intervention (except an optional approval gate before payment).

[![License: MIT](https://img.shields.io/badge/License-MIT-purple.svg)](./LICENSE)
[![Built with Qwen](https://img.shields.io/badge/Powered%20by-Qwen%20AI-orange)](https://dashscope-intl.aliyuncs.com)
[![Deployed on Alibaba Cloud](https://img.shields.io/badge/Deployed%20on-Alibaba%20Cloud-orange)](https://www.alibabacloud.com)

---

## Demo

[![OpenWorld Demo](https://img.youtube.com/vi/A2_0xyCH6Ls/maxresdefault.jpg)](https://youtu.be/A2_0xyCH6Ls)

| | |
|---|---|
| **Live Frontend** | [openworld-frontend.vercel.app](https://openworld-frontend.vercel.app) |
| **Live API** | `https://qwenhack-rdypqsjofu.us-west-1.fcapp.run/health` |
| **Track** | AI Agents & Automation |

---

## Problems We Solve

### 1. Travel Planning Takes 4–8 Hours of Manual Work

Booking an international trip requires juggling 6+ platforms — flights, hotels, transport, currency conversion, itinerary coordination, and budget tracking. Each step is manual, error-prone, and disconnected.

With OpenWorld:
- A single `trip.md` policy file replaces all input forms
- AI agents research, rank, and book automatically
- Budget constraints are enforced at every step — no overspending

### 2. Failures Have No Recovery

When a flight is full or a hotel is unavailable, users start over from scratch. There is no automated fallback — every failure costs hours of replanning.

With OpenWorld:
- `RecoveryAgent` detects every failure automatically
- Re-prompts Qwen with failure context to generate a different strategy (different airline, hotel tier, or route)
- Pipeline continues without user intervention

### 3. No Visibility Into What AI Is Doing

Most AI travel tools are black boxes — you submit a request and wait for a result. If something goes wrong, you don't know where or why.

With OpenWorld:
- Every agent emits structured logs streamed to the frontend in real time
- `PipelineGraph` component shows exactly which agent is running, completed, or failed
- Full activity feed with timestamps and agent names
- All logs persisted to **Alibaba SLS** for audit and debugging

### 4. Critical Spend Decisions Are Made Without Human Input

Autonomous systems that commit real spend without a human checkpoint are a trust barrier for adoption.

With OpenWorld:
- `VaultAgent` pauses the pipeline before any reservation is made
- Frontend renders an `ApprovalGate` showing exact budget breakdown
- Human approves or rejects via a single click — pipeline resumes or cancels
- Implemented as a `tokio::sync::oneshot` channel mid-pipeline pause, not a polling workaround

---

## Alibaba Cloud Services

| Service | Usage | Code Reference |
|---|---|---|
| **Qwen AI** (`qwen3.7-max`) | Powers all 8 agents — planning, search synthesis, recovery, report generation | [`src/og_compute.rs`](./backend/src/og_compute.rs) |
| **OSS** (Object Storage) | Stores final Markdown travel reports per session | [`src/agents/artifact.rs`](./backend/src/agents/artifact.rs) |
| **SLS** (Log Service) | Streams all agent activity logs to cloud logstore in real time | [`src/orchestrator.rs`](./backend/src/orchestrator.rs) |
| **Function Compute** | Hosts the Rust API as a serverless `linux/amd64` container | [`Dockerfile`](./backend/Dockerfile) |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     FRONTEND (React + Vite)                      │
│  TripEditor (Monaco) → useSession hook → REST polling 1.5s       │
│  PipelineGraph · ActivityFeed · ApprovalGate · TripResult        │
└───────────────────────────┬─────────────────────────────────────┘
                            │ HTTPS
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│           BACKEND — Rust / Axum (Alibaba Function Compute)       │
│                                                                   │
│  ┌─────────┐  ┌────────┐  ┌──────────┐  ┌────────────────────┐ │
│  │ Intent  │→ │Planner │→ │  Search  │→ │   Reservation      │ │
│  │ Parser  │  │ Agent  │  │  Agent   │  │   Agent            │ │
│  └─────────┘  └────────┘  └──────────┘  └─────────┬──────────┘ │
│                                                     │            │
│  ┌──────────┐  ┌──────────┐  ┌─────────┐           │            │
│  │ Artifact │← │ Recovery │← │  Vault  │←──────────┘            │
│  │  Agent   │  │  Agent   │  │  Agent  │  ← approval gate        │
│  └────┬─────┘  └──────────┘  └─────────┘                        │
│       │                                                           │
└───────┼───────────────────────────────────────────────────────── ┘
        │                    │                    │
        ▼                    ▼                    ▼
┌─────────────┐    ┌──────────────────┐  ┌─────────────────────┐
│  Qwen AI    │    │   Alibaba OSS    │  │   Alibaba SLS       │
│ qwen3.7-max │    │  (report store)  │  │  (activity logs)    │
└─────────────┘    └──────────────────┘  └─────────────────────┘
```

---

## Agent Pipeline

| Step | Agent | Role |
|---|---|---|
| 00 | **Intent** | Parses `trip.md` YAML into structured constraints and budget rules |
| 01 | **Planner** | Generates a day-by-day itinerary using Qwen AI |
| 02 | **Search** | Finds real flights, hotels, and transport via SerpAPI + Mapbox |
| 03 | **Reservation** | Selects optimal options and secures bookings |
| 04 | **Vault** | Enforces budget policy — pauses pipeline for human approval if over threshold |
| 05 | **Recovery** | Auto-retries failed bookings with alternative strategies |
| 06 | **Memory** | Persists session context and agent state across the pipeline |
| 07 | **Artifact** | Generates Markdown report, uploads to OSS, signs with HMAC-SHA256 proof |

---

## Pipeline Flow

```
User writes trip.md
       │
       ▼
POST /sessions  ──► create session (UUID)
       │
       ▼
POST /sessions/:id/start  ──► tokio::spawn(run_session)
       │
       ├─► PlannerAgent    ──► Qwen prompt → JSON itinerary
       │
       ├─► SearchAgent     ──► SerpAPI flights + hotels → Qwen ranking
       │
       ├─► VaultAgent      ──► budget check
       │       │
       │       └─ over threshold? ──► oneshot::channel pause
       │               │                    │
       │               │            frontend ApprovalGate
       │               │                    │
       │               └──── approve ────────┘
       │
       ├─► ReservationAgent ──► select + confirm bookings
       │
       ├─► RecoveryAgent    ──► retry failed items via Qwen replanning
       │
       └─► ArtifactAgent    ──► Qwen writes report → OSS upload → HMAC sign
                │
                ▼
        GET /sessions/:id/report  ──► Markdown report from OSS
```

---

## Key Code: Qwen Integration

Each agent calls Qwen with domain-specific prompts. Example — `PlannerAgent`:

```rust
// backend/src/agents/planner.rs
let prompt = format!(
    "You are a travel planner. Given this policy:\n{}\n\
     Generate a structured day-by-day itinerary as JSON with: \
     days[], each with activities[], estimated_cost_usd, and notes.",
    serde_yaml::to_string(&session.policy)?
);

let response = qwen_chat(&[
    Message { role: "system", content: "You are an expert travel planner." },
    Message { role: "user",   content: &prompt },
]).await?;
```

`RecoveryAgent` re-prompts with failure context:

```rust
// backend/src/agents/recovery.rs
let recovery_prompt = format!(
    "Previous booking attempt failed: {}\n\
     Original plan: {}\n\
     Suggest a completely different approach — \
     different airline, hotel tier, or route.",
    failure_reason, original_plan
);
```

---

## Key Code: Approval Gate (Human-in-the-Loop)

The pipeline pauses mid-execution using a `oneshot` channel:

```rust
// backend/src/agents/vault.rs
let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
session.set_approval_channel(tx).await;
session.set_state(SessionState::AwaitingApproval).await;

// Pipeline suspends here — waiting for HTTP POST /approve or /reject
let approved = rx.await.unwrap_or(false);

if !approved {
    session.set_state(SessionState::Failed).await;
    return Err(anyhow!("Budget rejected by operator"));
}
```

```rust
// backend/src/api.rs — POST /sessions/:id/approve
async fn approve_handler(...) {
    let sent = session.approve(true).await;  // sends true through oneshot
    // pipeline resumes from where it paused
}
```

---

## Key Code: HMAC Execution Proof

Every completed artifact is signed to prove tamper-free execution:

```rust
// backend/src/agents/artifact.rs
let preimage = format!(
    "{}|{}|{}",
    session.session_id,
    session.policy.to_constraint_json(),
    booking_refs.join(",")
);

let mut mac = HmacSha256::new_from_slice(operator_key.as_bytes())?;
mac.update(preimage.as_bytes());
let proof = hex::encode(mac.finalize().into_bytes());
```

Verify at any time: `GET /sessions/:id/verify`

---

## Live API

The backend is live on Alibaba Function Compute:

```bash
# Health check
curl https://qwenhack-rdypqsjofu.us-west-1.fcapp.run/health
# → {"status":"ok","service":"OpenWorld Agentic Travel API","version":"0.1.0"}

# Create a session
curl -X POST https://qwenhack-rdypqsjofu.us-west-1.fcapp.run/sessions \
  -H "Content-Type: application/json" \
  -d '{"travel_md": "trip:\n  origin: BKK\n  destination: TYO\n  budget_max: \"1500 USD\"\n  departure_date: \"2026-08-01\"\n  return_date: \"2026-08-06\""}'

# Start pipeline
curl -X POST https://qwenhack-rdypqsjofu.us-west-1.fcapp.run/sessions/<id>/start

# Stream logs
curl https://qwenhack-rdypqsjofu.us-west-1.fcapp.run/sessions/<id>/logs
```

---

## Technical Depth & Engineering

OpenWorld makes deep, non-trivial use of **Qwen AI (qwen3.7-max)** throughout the pipeline — not just a single prompt, but a multi-agent system where each agent has a distinct Qwen prompt strategy:

- **PlannerAgent** sends structured system + user prompts to Qwen to produce a JSON itinerary from a YAML policy — with budget constraints embedded in the prompt context
- **SearchAgent** synthesises Qwen reasoning over raw SerpAPI results (flights, hotels) to rank and select the best options
- **RecoveryAgent** re-prompts Qwen with failure context and previous attempt data to generate an alternative strategy
- **ArtifactAgent** uses Qwen to write a rich GFM Markdown travel report with cost tables, day-by-day schedule, and booking summaries

Engineering innovations:
- **Async Rust state machine** — `SessionState` enum drives the full pipeline via `tokio::spawn`, with `broadcast` channels for real-time log streaming to multiple subscribers
- **HMAC-SHA256 execution proof** — every artifact is cryptographically signed over `session_id | policy_constraints | booking_refs` to prove tamper-free execution
- **Mid-pipeline approval gate** — `tokio::sync::oneshot` channel pauses execution awaiting a human HTTP decision, then resumes exactly where it stopped
- **Serverless container on FC** — `linux/amd64` Docker container on Alibaba Function Compute, cold-started on demand with 300s timeout

---

## Innovation & AI Creativity

The architecture treats **AI agents as stateful typed pipeline stages**, not one-shot chatbots:

- Each agent receives structured context (policy + previous agent outputs) and produces typed output consumed by the next stage — a **typed AI pipeline** rather than a chat loop
- The **Intent Agent** translates human-readable YAML policy into machine-executable constraints that every downstream agent respects — separating intent from execution
- **RecoveryAgent** implements a novel retry strategy: instead of replaying the same failed action, it re-prompts Qwen with the failure reason and asks for a fundamentally different approach
- The **approval gate** is a first-class architectural primitive — not a UI afterthought — a mid-pipeline pause with exact budget breakdown surfaced to the human decision UI
- The frontend **PipelineGraph** derives each node's state from live log stream analysis rather than explicit state events — resilient to partial failures

Tech stack: Rust + Axum (async, zero-cost abstractions), React 18 + Framer Motion (real-time animated pipeline), Monaco Editor (in-browser YAML), react-markdown + remark-gfm (AI report rendering).

---

## Problem Value & Impact

**The problem:** Planning and booking a multi-day international trip requires 4–8 hours of manual research across 6+ platforms — flights, hotels, transport, budget tracking, itinerary coordination.

**OpenWorld's solution:** A single `trip.md` file replaces all of that:
1. Understands your constraints (budget, preferences, dates) as **policy — not a chat message**
2. Executes against real travel APIs with AI-powered selection
3. Recovers from failures automatically without user intervention
4. Asks for human approval only at the critical payment decision point
5. Delivers a complete formatted travel report stored in Alibaba OSS

**Real-world relevance:** This architecture mirrors enterprise travel management systems (Concur, TripActions) but built as open-source, API-driven infrastructure. The policy-as-code (`trip.md`) approach is programmable — companies could define corporate travel policies as YAML and have every employee booking automatically enforce them.

**Scalability potential:** The agent pipeline is stateless per session and runs on serverless FC — horizontal scaling is automatic. The `trip.md` policy format is extensible to any domain requiring policy-driven multi-step AI orchestration (expense management, procurement, event planning).

---

## Presentation & Documentation

- **Demo Video** — [YouTube 3-minute walkthrough](https://youtu.be/A2_0xyCH6Ls) showing the full pipeline from `trip.md` to final AI report
- **Live Frontend** — [openworld-frontend.vercel.app](https://openworld-frontend.vercel.app) — interactive trip editor with real-time pipeline graph
- **Live API** — `https://qwenhack-rdypqsjofu.us-west-1.fcapp.run` — Alibaba FC deployed backend, health-checked and running
- **Architecture** — ASCII diagram above + component-level breakdown in Project Structure

---

## The `trip.md` Format

```yaml
trip:
  origin: BKK                    # IATA departure code
  destination: TYO               # IATA destination code
  departure_date: "2026-08-01"
  return_date:    "2026-08-06"
  budget_max: "1500 USD"

flight:
  max_stops: 1
  avoid_red_eye: true
  preferred_airlines: [ANA, JAL]

hotel:
  min_rating: 4.0
  max_price_per_night: "120 USD"
  near_station: true

vault:
  auto_payment: true
  max_single_transaction: "800 USD"  # triggers approval gate if exceeded
```

---

## Project Structure

```
openworld/
├── backend/                    # Rust orchestration engine + API server
│   ├── src/
│   │   ├── agents/
│   │   │   ├── planner.rs      # Qwen itinerary generation
│   │   │   ├── search.rs       # SerpAPI + Qwen ranking
│   │   │   ├── reservation.rs  # Booking selection + confirmation
│   │   │   ├── vault.rs        # Budget enforcement + approval gate
│   │   │   ├── recovery.rs     # Failure detection + Qwen replanning
│   │   │   └── artifact.rs     # OSS upload + HMAC proof signing
│   │   ├── api.rs              # Axum HTTP + WebSocket server
│   │   ├── og_compute.rs       # Qwen AI client
│   │   ├── og_storage.rs       # Alibaba OSS client
│   │   ├── orchestrator.rs     # Session state machine + SLS logging
│   │   └── travel_spec.rs      # trip.md YAML parser
│   ├── Dockerfile              # linux/amd64 for Alibaba FC
│   └── examples/trip.md        # Example trip policy
├── frontend/                   # React 18 + TypeScript + Vite
│   └── src/
│       ├── hooks/useSession.ts     # API polling + state
│       └── components/
│           ├── PipelineGraph.tsx   # Live agent pipeline visualisation
│           ├── AgentShowcase.tsx   # 8-agent showcase section
│           ├── ApprovalGate.tsx    # Human-in-the-loop UI
│           ├── TripResult.tsx      # AI report renderer (react-markdown)
│           ├── ActivityFeed.tsx    # Real-time log stream
│           └── TripEditor.tsx      # Monaco editor for trip.md
└── README.md
```

---

## Local Development

```bash
# Backend
cd backend
cp .env.example .env      # fill in API keys
cargo run --bin api        # http://localhost:3000

# Frontend
cd frontend
pnpm install
echo "VITE_API_URL=http://localhost:3000" > .env.local
pnpm dev                   # http://localhost:5173

# CLI (no frontend needed)
cd backend
cargo run --bin travel -- examples/trip.md
```

---

## Environment Variables

| Variable | Description |
|---|---|
| `QWEN_API_KEY` | Alibaba Cloud Model Studio API key |
| `QWEN_ENDPOINT` | Qwen chat completions URL |
| `QWEN_MODEL` | Model name (`qwen3.7-max`) |
| `SERPAPI_KEY` | SerpAPI key for flight/hotel search |
| `MAPBOX_ACCESS_TOKEN` | Mapbox token for location data |
| `OSS_ACCESS_KEY_ID` | Alibaba Cloud OSS access key |
| `OSS_ACCESS_KEY_SECRET` | Alibaba Cloud OSS secret |
| `OSS_BUCKET` | OSS bucket name |
| `OSS_ENDPOINT` | OSS regional endpoint |
| `SLS_ACCESS_KEY_ID` | Alibaba SLS access key |
| `SLS_ACCESS_KEY_SECRET` | Alibaba SLS secret |
| `SLS_ENDPOINT` | SLS endpoint |
| `SLS_PROJECT` | SLS project name |
| `SLS_LOGSTORE` | SLS logstore name |

---

## Deployment

```bash
# Build + push to Docker Hub
cd backend
docker buildx build --platform linux/amd64 \
  -t jfkongphop/openworld-api:latest --push .

# Redeploy on FC: Console → Edit Function → Deploy
```

FC settings: Custom Container · `./api` · Port `3000` · Timeout `300s`

---

## License

MIT © 2026 OpenWorld — see [LICENSE](./LICENSE)
