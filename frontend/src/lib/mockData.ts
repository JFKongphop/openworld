// ── Mock data for realtime simulation ─────────────────────────────────────

export const MOCK_TRIP_MD = `trip:
  owner: "0x874604c87A1FEF538Ce21192aac0Db131F5F24ae"
  origin: BKK
  destination: TYO
  departure_date: "2026-06-10"
  return_date: "2026-06-15"
  budget_max: "1500 USD"

flight:
  max_stops: 1
  avoid_red_eye: true
  preferred_airlines:
    - ANA
    - JAL
    - Singapore Airlines
    - Emirates

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
`

export type AgentName =
  | 'PlannerAgent'
  | 'SearchAgent'
  | 'ReservationAgent'
  | 'VaultAgent'
  | 'ArtifactAgent'
  | 'System'

export interface ActivityEvent {
  id: string
  agent: AgentName
  message: string
  timestamp: string
  status: 'running' | 'success' | 'error' | 'info'
}

export const INITIAL_ACTIVITY: ActivityEvent[] = [
  { id: '1', agent: 'System', message: 'Session initialised — policy parsed from trip.md', timestamp: '14:21:03', status: 'info' },
  { id: '2', agent: 'PlannerAgent', message: 'Generating itinerary for BKK → TYO (5 days)', timestamp: '14:21:04', status: 'running' },
  { id: '3', agent: 'PlannerAgent', message: 'Itinerary generated — 7 segments identified', timestamp: '14:21:07', status: 'success' },
  { id: '4', agent: 'SearchAgent', message: 'Searching flights ANA/JAL BKK→NRT in parallel', timestamp: '14:21:08', status: 'running' },
  { id: '5', agent: 'SearchAgent', message: 'Flight ANA NH908 found — ¥89,400 ($612 USD)', timestamp: '14:21:11', status: 'success' },
  { id: '6', agent: 'SearchAgent', message: 'Hotel search near Shinjuku station — 4★ min', timestamp: '14:21:12', status: 'running' },
  { id: '7', agent: 'SearchAgent', message: 'Hotel Hyatt Regency Tokyo selected — $98/night', timestamp: '14:21:15', status: 'success' },
]

export const STREAMING_EVENTS: Omit<ActivityEvent, 'id' | 'timestamp'>[] = [
  { agent: 'ReservationAgent', message: 'Booking ANA NH908 — seat 23A confirmed', status: 'success' },
  { agent: 'ReservationAgent', message: 'Hotel reservation submitted — conf #HTR-88291', status: 'success' },
  { agent: 'VaultAgent', message: 'Vault balance check — 1.42 ETH available', status: 'info' },
  { agent: 'VaultAgent', message: 'Payment approved — $612 USD flight', status: 'success' },
  { agent: 'VaultAgent', message: 'Payment approved — $490 USD hotel (5 nights)', status: 'success' },
  { agent: 'SearchAgent', message: 'Day activities search — Tokyo highlights', status: 'running' },
  { agent: 'SearchAgent', message: 'Shinjuku Gyoen, TeamLab, Tsukiji market added', status: 'success' },
  { agent: 'ArtifactAgent', message: 'Generating markdown travel report...', status: 'running' },
  { agent: 'ArtifactAgent', message: 'Report generated — 2,847 words', status: 'success' },
  { agent: 'ArtifactAgent', message: 'Uploading to 0G Storage...', status: 'running' },
  { agent: 'ArtifactAgent', message: 'Uploaded — root hash 0x5feb...587f', status: 'success' },
  { agent: 'ArtifactAgent', message: 'Minting ERC-7857 on 0G Galileo testnet...', status: 'running' },
  { agent: 'ArtifactAgent', message: 'NFT minted — tokenId #1 — tx 0xcc68...d82', status: 'success' },
  { agent: 'System', message: 'Journey complete — all segments confirmed on-chain', status: 'success' },
]

export interface RootHash {
  id: string
  filename: string
  hash: string
  type: 'REPORT' | 'ARTIFACT' | 'MEMORY' | 'LOG'
  size: string
  uploadedAgo: string
  txHash: string
}

export const ROOT_HASHES: RootHash[] = [
  {
    id: '1',
    filename: 'trip-report-tokyo-jun2026.md',
    hash: '0x5febd814d8e4c5a7d058da967deb26f6118c2faa656e82a11abd2f37f319587f',
    type: 'REPORT',
    size: '18.4 KB',
    uploadedAgo: '2m ago',
    txHash: '0xcc682576d3206bf6a7a3000f0cd59b2ef200f75a9fbf4b46f4fa05f6ded18d82',
  },
  {
    id: '2',
    filename: 'journey-memory-449cc38a.json',
    hash: '0x76551e37bc5df5d121e4d8756d0859d5d541d396b0355eeb40917f93c12c38c1',
    type: 'MEMORY',
    size: '64.2 KB',
    uploadedAgo: '2m ago',
    txHash: '0xcc682576d3206bf6a7a3000f0cd59b2ef200f75a9fbf4b46f4fa05f6ded18d82',
  },
]

export interface TxEvent {
  id: string
  action: string
  hash: string
  status: 'success' | 'pending' | 'failed'
  block: number
  timestamp: string
}

export const TX_EVENTS: TxEvent[] = [
  { id: '1', action: 'Mint ERC-7857', hash: '0xcc682576d3206bf6a...d82', status: 'success', block: 33642644, timestamp: '14:23:17' },
  { id: '2', action: 'Upload Report Root', hash: '0x5febd814d8e4c5a7...87f', status: 'success', block: 33642640, timestamp: '14:23:09' },
  { id: '3', action: 'Upload Memory Root', hash: '0x76551e37bc5df5d1...8c1', status: 'success', block: 33642635, timestamp: '14:23:01' },
]

export interface JourneyArtifact {
  id: string
  destination: string
  origin: string
  dates: string
  totalSpent: string
  tokenId: number
  txHash: string
  reportHash: string
  sessionId: string
  segments: number
}

export const JOURNEY_ARTIFACTS: JourneyArtifact[] = [
  {
    id: '1',
    destination: 'Tokyo',
    origin: 'Bangkok',
    dates: 'Jun 10 – Jun 15, 2026',
    totalSpent: '$985 USD',
    tokenId: 1,
    txHash: '0xcc682576d3206bf6a7a3000f0cd59b2ef200f75a9fbf4b46f4fa05f6ded18d82',
    reportHash: '0x5febd814d8e4c5a7d058da967deb26f6118c2faa656e82a11abd2f37f319587f',
    sessionId: '449cc38a-36bd-4d00-ae6a-ccfd9bbf81a7',
    segments: 7,
  },
]
