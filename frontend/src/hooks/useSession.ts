/**
 * useSession — wires the frontend to the OpenWorld backend API.
 *
 * Flow:
 *   runPipeline(travelMd)
 *     → POST /sessions          (create)
 *     → POST /sessions/:id/start (launch 7-agent pipeline)
 *     → polls GET /sessions/:id/logs every 1.5s
 *     → polls GET /sessions/:id  for state + artifact
 *     → resolves when state === "complete" | "failed"
 */

import { useState, useEffect, useRef, useCallback } from 'react'

const API_BASE = (import.meta.env.VITE_API_URL as string | undefined) ?? 'http://localhost:3000'

// ── Types matching the backend ────────────────────────────────────────────────

export interface BackendLog {
  timestamp: string
  agent: string
  message: string
  log_type: 'info' | 'success' | 'warning' | 'error' | 'action'
}

export interface BackendArtifact {
  artifact_id: string
  session_id: string
  trip_summary: string
  destination: string
  duration_days: number
  total_spent_usd: number
  execution_proof: string | null
  storage_root_hash: string | null
  report_root_hash: string | null
  report_path: string | null
  created_at: string
}

// ── Hook ─────────────────────────────────────────────────────────────────────

export function useSession() {
  const [sessionId, setSessionId]   = useState<string | null>(null)
  const [logs, setLogs]             = useState<BackendLog[]>([])
  const [pipelineState, setPipelineState] = useState<string>('idle')
  const [isRunning, setIsRunning]   = useState(false)
  const [needsApproval, setNeedsApproval] = useState(false)
  const [artifact, setArtifact]     = useState<BackendArtifact | null>(null)
  const [reportMd, setReportMd]     = useState<string | null>(null)
  const [error, setError]           = useState<string | null>(null)
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current)
      pollRef.current = null
    }
  }, [])

  const pollSession = useCallback(async (id: string) => {
    try {
      const [logsRes, sessionRes] = await Promise.all([
        fetch(`${API_BASE}/sessions/${id}/logs`),
        fetch(`${API_BASE}/sessions/${id}`),
      ])

      if (logsRes.ok) {
        const newLogs: BackendLog[] = await logsRes.json()
        setLogs(newLogs)
      }

      if (sessionRes.ok) {
        const session = await sessionRes.json()
        setPipelineState(session.state as string)
        setNeedsApproval(session.state === 'awaiting_approval')

        if (session.state === 'complete' || session.state === 'failed') {
          setIsRunning(false)
          stopPolling()
          if (session.artifact) {
            setArtifact(session.artifact as BackendArtifact)
          }
          // Fetch the markdown report
          try {
            const reportRes = await fetch(`${API_BASE}/sessions/${id}/report`)
            if (reportRes.ok) {
              const text = await reportRes.text()
              setReportMd(text)
            }
          } catch { /* report fetch is best-effort */ }
        }
      }
    } catch {
      // API unreachable — stop polling silently
      stopPolling()
      setIsRunning(false)
    }
  }, [stopPolling])

  const runPipeline = useCallback(async (travelMd: string) => {
    setLogs([])
    setArtifact(null)
    setReportMd(null)
    setNeedsApproval(false)
    setError(null)
    setPipelineState('creating')
    setIsRunning(true)

    try {
      // 1. Create session
      const createRes = await fetch(`${API_BASE}/sessions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ travel_md: travelMd }),
      })
      if (!createRes.ok) throw new Error('Failed to create session')
      const { session_id } = await createRes.json() as { session_id: string }
      setSessionId(session_id)

      // 2. Launch pipeline
      const startRes = await fetch(`${API_BASE}/sessions/${session_id}/start`, {
        method: 'POST',
      })
      if (!startRes.ok) throw new Error('Failed to start pipeline')
      setPipelineState('planning')

      // 3. Poll logs + state every 1.5s
      stopPolling()
      pollRef.current = setInterval(() => pollSession(session_id), 1500)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      setError(msg)
      setIsRunning(false)
      setPipelineState('error')
    }
  }, [pollSession, stopPolling])

  const approve = useCallback(async () => {
    if (!sessionId) return
    setNeedsApproval(false)
    await fetch(`${API_BASE}/sessions/${sessionId}/approve`, { method: 'POST' })
  }, [sessionId])

  const reject = useCallback(async () => {
    if (!sessionId) return
    setNeedsApproval(false)
    setIsRunning(false)
    stopPolling()
    await fetch(`${API_BASE}/sessions/${sessionId}/reject`, { method: 'POST' })
    setPipelineState('failed')
  }, [sessionId, stopPolling])

  // Cleanup on unmount
  useEffect(() => () => stopPolling(), [stopPolling])

  return { sessionId, logs, pipelineState, isRunning, needsApproval, artifact, reportMd, error, runPipeline, approve, reject }
}
