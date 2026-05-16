import { useEffect, useState } from 'react'
import { ethers } from 'ethers'
import type { RootHash, TxEvent, JourneyArtifact } from '../lib/mockData'

const RPC_URL     = 'https://evmrpc-testnet.0g.ai'
const CONTRACT    = '0xAF2699e9d306b57F5541aE3f04C43586589fD455'
const FROM_BLOCK  = 33640000

const ABI = [
  'event ReportPublished(address indexed owner, bytes32 indexed reportHash, uint256 indexed tokenId, string sessionId)',
  'function journeyMeta(uint256 tokenId) view returns (string sessionId, string tripDescription, uint256 createdAt, uint256 completedAt)',
  'function memoryOf(uint256 tokenId) view returns (tuple(string dataDescription, bytes32 dataHash)[])',
  'function reportOf(uint256 tokenId) view returns (tuple(string dataDescription, bytes32 dataHash)[])',
]

// "BKK → TYO (5d) — 985 USD spent on 7 bookings"
function parseTripDesc(desc: string) {
  const iata: Record<string, string> = {
    TYO: 'Tokyo', NRT: 'Tokyo', HND: 'Tokyo',
    OSA: 'Osaka', KIX: 'Osaka',
    BKK: 'Bangkok', DMK: 'Bangkok',
    SIN: 'Singapore',
    CDG: 'Paris', PAR: 'Paris',
    LHR: 'London', LON: 'London',
    DXB: 'Dubai',
    SYD: 'Sydney',
    NYC: 'New York', JFK: 'New York',
    LAX: 'Los Angeles',
  }
  const arrowMatch = desc.match(/^([A-Z]{2,4})\s*[→\-–>]+\s*([A-Z]{2,4})/)
  const origin      = arrowMatch?.[1] ?? '???'
  const destCode    = arrowMatch?.[2] ?? '???'
  const destination = iata[destCode] ?? destCode

  const spentMatch = desc.match(/([\d,]+)\s*USD\s*spent/)
  const totalSpent = spentMatch ? `$${spentMatch[1]} USD` : 'N/A'

  const segMatch = desc.match(/(\d+)\s*booking/)
  const segments = segMatch ? parseInt(segMatch[1]) : 0

  return { origin, destination, totalSpent, segments }
}

function timeAgo(unixTs: number): string {
  const diff = Math.floor(Date.now() / 1000) - unixTs
  if (diff < 60)   return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

function shortHash(h: string, a = 10, b = 6) {
  return `${h.slice(0, a)}...${h.slice(-b)}`
}

export interface ChainData {
  rootHashes: RootHash[]
  txEvents: TxEvent[]
  artifacts: JourneyArtifact[]
  loading: boolean
  error: string | null
}

export function useChainData(): ChainData {
  const [state, setState] = useState<ChainData>({
    rootHashes: [],
    txEvents: [],
    artifacts: [],
    loading: true,
    error: null,
  })

  useEffect(() => {
    let cancelled = false

    async function load() {
      try {
        const provider = new ethers.JsonRpcProvider(RPC_URL)
        const contract = new ethers.Contract(CONTRACT, ABI, provider)

        // ── 1. Fetch all ReportPublished events ───────────────────────────
        const filter    = contract.filters.ReportPublished()
        const rawLogs   = await contract.queryFilter(filter, FROM_BLOCK, 'latest')

        if (cancelled) return

        const rootHashes: RootHash[] = []
        const txEvents:   TxEvent[]  = []
        const artifacts:  JourneyArtifact[] = []

        await Promise.all(
          rawLogs.map(async (log) => {
            if (!('args' in log)) return
            const { owner, reportHash, tokenId, sessionId } = log.args as {
              owner: string
              reportHash: string
              tokenId: bigint
              sessionId: string
            }

            const txHash     = log.transactionHash
            const blockNum   = log.blockNumber
            const tokenIdNum = Number(tokenId)

            // ── 2. Fetch journeyMeta + memory/report collections ───────────
            const [meta, memDatas, repDatas, block] = await Promise.all([
              contract.journeyMeta(tokenId),
              contract.memoryOf(tokenId),
              contract.reportOf(tokenId),
              provider.getBlock(blockNum),
            ])

            const createdAt  = block?.timestamp ?? 0
            const uploadedAgo = timeAgo(Number(meta.createdAt) || createdAt)

            // Root hashes — report
            repDatas.forEach((d: { dataDescription: string; dataHash: string }, i: number) => {
              rootHashes.push({
                id:          `rep-${tokenIdNum}-${i}`,
                filename:    `trip-report-${sessionId.slice(0, 8)}.md`,
                hash:        d.dataHash,
                type:        'REPORT',
                size:        '—',
                uploadedAgo,
                txHash,
              })
            })

            // Root hashes — memory
            memDatas.forEach((d: { dataDescription: string; dataHash: string }, i: number) => {
              rootHashes.push({
                id:          `mem-${tokenIdNum}-${i}`,
                filename:    `journey-memory-${sessionId.slice(0, 8)}.json`,
                hash:        d.dataHash,
                type:        'MEMORY',
                size:        '—',
                uploadedAgo,
                txHash,
              })
            })

            // TX events
            txEvents.push({
              id:        `tx-${txHash}`,
              action:    'Mint ERC-7857',
              hash:      shortHash(txHash),
              status:    'success',
              block:     blockNum,
              timestamp: new Date(createdAt * 1000).toLocaleTimeString(),
            })

            // Journey artifact
            const { origin, destination, totalSpent, segments } = parseTripDesc(meta.tripDescription)
            artifacts.push({
              id:          String(tokenIdNum),
              destination,
              origin,
              dates:       new Date(createdAt * 1000).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' }),
              totalSpent,
              tokenId:     tokenIdNum,
              txHash,
              reportHash:  repDatas[0]?.dataHash ?? reportHash,
              sessionId,
              segments,
            })
          })
        )

        if (cancelled) return

        // Sort newest first
        txEvents.sort((a, b) => b.block - a.block)
        artifacts.sort((a, b) => b.tokenId - a.tokenId)

        setState({ rootHashes, txEvents, artifacts, loading: false, error: null })
      } catch (err) {
        if (!cancelled) {
          setState((s) => ({ ...s, loading: false, error: String(err) }))
        }
      }
    }

    load()
    return () => { cancelled = true }
  }, [])

  return state
}
