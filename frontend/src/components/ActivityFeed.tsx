import { useState, useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { INITIAL_ACTIVITY, STREAMING_EVENTS, type ActivityEvent, type AgentName } from '../lib/mockData'

const AGENT_COLORS: Record<AgentName, string> = {
  PlannerAgent:     'text-purple-600 bg-purple-50 border-purple-200',
  SearchAgent:      'text-blue-600 bg-blue-50 border-blue-200',
  ReservationAgent: 'text-indigo-600 bg-indigo-50 border-indigo-200',
  VaultAgent:       'text-emerald-600 bg-emerald-50 border-emerald-200',
  ArtifactAgent:    'text-pink-600 bg-pink-50 border-pink-200',
  System:           'text-gray-600 bg-gray-50 border-gray-200',
}

const STATUS_DOT: Record<ActivityEvent['status'], string> = {
  running: 'bg-yellow-400 animate-pulse',
  success: 'bg-green-500',
  error:   'bg-red-500',
  info:    'bg-purple-400',
}

let idCounter = 100

export default function ActivityFeed() {
  const [events, setEvents] = useState<ActivityEvent[]>(INITIAL_ACTIVITY)
  const [streamIdx, setStreamIdx] = useState(0)
  const [isStreaming, setIsStreaming] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)

  // Auto-scroll within feed container only
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }, [events])

  // Stream events every 2.5s after mount
  useEffect(() => {
    if (streamIdx >= STREAMING_EVENTS.length) return
    const delay = isStreaming ? 2500 : 3000
    const tid = setTimeout(() => {
      const next = STREAMING_EVENTS[streamIdx]
      const now = new Date()
      const ts = `${now.getHours().toString().padStart(2,'0')}:${now.getMinutes().toString().padStart(2,'0')}:${now.getSeconds().toString().padStart(2,'0')}`
      setEvents((prev) => [
        ...prev,
        { ...next, id: String(++idCounter), timestamp: ts },
      ])
      setStreamIdx((i) => i + 1)
      setIsStreaming(true)
    }, delay)
    return () => clearTimeout(tid)
  }, [streamIdx, isStreaming])

  return (
    <div className="glass rounded-3xl flex flex-col h-full shadow-xl shadow-purple-200/30 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-purple-100/60">
        <div>
          <h3 className="font-grotesk font-bold text-base text-purple-950">Agent Activity</h3>
          <p className="text-xs text-purple-400 mt-0.5">Live orchestration feed</p>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          <span className="text-xs font-semibold text-green-600">LIVE</span>
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
                <div className={`w-2 h-2 rounded-full ${STATUS_DOT[evt.status]}`} />
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-0.5">
                  <span className={`text-xs font-semibold px-2 py-0.5 rounded-full border ${AGENT_COLORS[evt.agent]}`}>
                    {evt.agent}
                  </span>
                  <span className="text-xs text-purple-300 font-mono">{evt.timestamp}</span>
                </div>
                <p className="text-sm text-purple-800 leading-snug">{evt.message}</p>
              </div>
            </motion.div>
          ))}
        </AnimatePresence>
        <div ref={bottomRef} />
      </div>

      {/* Footer */}
      <div className="px-5 py-2.5 border-t border-purple-100/60 bg-purple-50/30 flex items-center justify-between">
        <span className="text-xs text-purple-400">{events.length} events</span>
        <span className="text-xs text-purple-400">
          {streamIdx < STREAMING_EVENTS.length ? (
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-yellow-400 animate-pulse" />
              Streaming…
            </span>
          ) : (
            <span className="text-green-600 font-medium">Complete</span>
          )}
        </span>
      </div>
    </div>
  )
}
