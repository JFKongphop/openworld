import { useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { INITIAL_ACTIVITY, type ActivityEvent } from '../lib/mockData'
import type { BackendLog } from '../hooks/useSession'

const AGENT_COLORS: Record<string, string> = {
  PlannerAgent:     'text-purple-600 bg-purple-50 border-purple-200',
  SearchAgent:      'text-blue-600 bg-blue-50 border-blue-200',
  ReservationAgent: 'text-indigo-600 bg-indigo-50 border-indigo-200',
  VaultAgent:       'text-emerald-600 bg-emerald-50 border-emerald-200',
  ArtifactAgent:    'text-pink-600 bg-pink-50 border-pink-200',
  RecoveryAgent:    'text-orange-600 bg-orange-50 border-orange-200',
  Orchestrator:     'text-gray-600 bg-gray-50 border-gray-200',
  System:           'text-gray-600 bg-gray-50 border-gray-200',
}

const LOG_TYPE_DOT: Record<string, string> = {
  success: 'bg-green-500',
  error:   'bg-red-500',
  warning: 'bg-yellow-500',
  action:  'bg-yellow-400 animate-pulse',
  info:    'bg-purple-400',
}

function backendToEvent(log: BackendLog, idx: number): ActivityEvent {
  const statusMap: Record<string, ActivityEvent['status']> = {
    success: 'success',
    error:   'error',
    warning: 'error',
    action:  'running',
    info:    'info',
  }
  return {
    id: String(1000 + idx),
    agent: log.agent as ActivityEvent['agent'],
    message: log.message,
    timestamp: log.timestamp,
    status: statusMap[log.log_type] ?? 'info',
  }
}

interface Props {
  logs?: BackendLog[]
  isRunning?: boolean
  needsApproval?: boolean
}

export default function ActivityFeed({ logs, isRunning, needsApproval }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null)

  // Use real logs if available, fall back to mock data
  const events: ActivityEvent[] = logs && logs.length > 0
    ? logs.map(backendToEvent)
    : INITIAL_ACTIVITY

  // Auto-scroll to bottom as new logs arrive
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }, [events.length])

  return (
    <div className="glass rounded-3xl flex flex-col h-full shadow-xl shadow-purple-200/30 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-purple-100/60">
        <div>
          <h3 className="font-grotesk font-bold text-base text-purple-950">Live Workflow Execution</h3>
          <p className="text-xs text-purple-400 mt-0.5">Live orchestration feed</p>
        </div>
        <div className="flex items-center gap-2">
          <span className={`w-2 h-2 rounded-full ${
            needsApproval ? 'bg-orange-500 animate-pulse'
            : isRunning   ? 'bg-yellow-400 animate-pulse'
            : 'bg-green-500'
          }`} />
          <span className={`text-xs font-semibold ${
            needsApproval ? 'text-orange-600'
            : isRunning   ? 'text-yellow-600'
            : 'text-green-600'
          }`}>
            {needsApproval ? 'PAUSED' : isRunning ? 'RUNNING' : 'LIVE'}
          </span>
        </div>
      </div>

      {/* Events */}
      <div className="flex-1 overflow-y-auto p-4 space-y-2 min-h-0">
        <AnimatePresence initial={false}>
          {events.map((evt) => (
            <motion.div
              key={evt.id}
              initial={{ opacity: 0, x: -12, scale: 0.97 }}
              animate={{ opacity: 1, x: 0, scale: 1 }}
              transition={{ duration: 0.35, ease: 'easeOut' }}
              className="flex items-start gap-3 p-3 rounded-xl bg-white/50 border border-white/70 hover:bg-white/70 transition-colors duration-150"
            >
              {/* Timeline dot */}
              <div className="flex flex-col items-center pt-1 flex-shrink-0">
                <div className={`w-2 h-2 rounded-full ${LOG_TYPE_DOT[evt.status] ?? 'bg-purple-400'}`} />
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-0.5">
                  <span className={`text-xs font-semibold px-2 py-0.5 rounded-full border ${AGENT_COLORS[evt.agent] ?? 'text-gray-600 bg-gray-50 border-gray-200'}`}>
                    {evt.agent}
                  </span>
                  <span className="text-xs text-purple-300 font-mono">{evt.timestamp}</span>
                </div>
                <p className="text-sm text-purple-800 leading-snug break-words line-clamp-3">{evt.message}</p>
              </div>
            </motion.div>
          ))}
        </AnimatePresence>
        <div ref={bottomRef} />
      </div>

      {/* Footer */}
      <div className="px-5 py-2.5 border-t border-purple-100/60 bg-purple-50/30 flex items-center justify-between">
        <span className="text-xs text-purple-400">{events.length} events</span>
        <span className="text-xs">
          {isRunning ? (
            <span className="flex items-center gap-1.5 text-yellow-600">
              <span className="w-1.5 h-1.5 rounded-full bg-yellow-400 animate-pulse" />
              Running pipeline…
            </span>
          ) : logs && logs.length > 0 ? (
            <span className="text-green-600 font-medium">Complete</span>
          ) : (
            <span className="text-purple-400">Demo mode</span>
          )}
        </span>
      </div>
    </div>
  )
}
