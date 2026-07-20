import { motion, AnimatePresence } from 'framer-motion'
import {
  Flag, Brain, Search, Calendar, DollarSign,
  FileText, CheckCircle, Loader2, AlertCircle, Circle,
} from 'lucide-react'
import type { BackendLog } from '../hooks/useSession'

// ── Types ────────────────────────────────────────────────────────────────────

type NodeState = 'pending' | 'running' | 'success' | 'error'

interface PipelineStep {
  id: string
  agent: string
  label: string
  typeLabel: string
  icon: React.ReactNode
  defaultMsg: string
}

// ── Pipeline definition ───────────────────────────────────────────────────────

const STEPS: PipelineStep[] = [
  {
    id: 'system',
    agent: 'System',
    label: 'Session Start',
    typeLabel: 'System',
    icon: <Flag size={13} />,
    defaultMsg: 'Initialise session and parse trip.md policy',
  },
  {
    id: 'planner',
    agent: 'PlannerAgent',
    label: 'Planner Agent',
    typeLabel: 'AI · Planning',
    icon: <Brain size={13} />,
    defaultMsg: 'Generate multi-day itinerary from policy constraints',
  },
  {
    id: 'search',
    agent: 'SearchAgent',
    label: 'Search Agent',
    typeLabel: 'AI · Search',
    icon: <Search size={13} />,
    defaultMsg: 'Search flights, hotels and activities in parallel',
  },
  {
    id: 'reservation',
    agent: 'ReservationAgent',
    label: 'Reservation Agent',
    typeLabel: 'AI · Booking',
    icon: <Calendar size={13} />,
    defaultMsg: 'Confirm and reserve all travel segments',
  },
  {
    id: 'vault',
    agent: 'VaultAgent',
    label: 'Vault Agent',
    typeLabel: 'AI · Payment',
    icon: <DollarSign size={13} />,
    defaultMsg: 'Authorise and process budget allocation',
  },
  {
    id: 'artifact',
    agent: 'ArtifactAgent',
    label: 'Artifact Agent',
    typeLabel: 'AI · Storage',
    icon: <FileText size={13} />,
    defaultMsg: 'Generate report and upload to cloud storage',
  },
]

// ── State helpers ─────────────────────────────────────────────────────────────

function getState(
  agent: string,
  logs: BackendLog[],
  nextAgent: string | null,
  pipelineComplete: boolean,
): NodeState {
  // Pipeline finished — every node is success
  if (pipelineComplete) return 'success'

  // If the next agent has already started, this one is done.
  // Checked BEFORE own-log check so nodes with zero own-logs still resolve.
  if (nextAgent) {
    const nextLogs = logs.filter(l => l.agent === nextAgent)
    if (nextLogs.length > 0) return 'success'
  }

  const al = logs.filter(l => l.agent === agent)
  if (!al.length) return 'pending'
  if (al.some(l => l.log_type === 'error')) return 'error'
  const last = al[al.length - 1]
  if (last.log_type === 'success') return 'success'
  return 'running'
}

function getMsg(agent: string, logs: BackendLog[], fallback: string): string {
  const al = logs.filter(l => l.agent === agent)
  return al.length ? al[al.length - 1].message : fallback
}

function getTs(agent: string, logs: BackendLog[]): string | null {
  const al = logs.filter(l => l.agent === agent)
  return al.length ? al[al.length - 1].timestamp : null
}

// ── Node styles ───────────────────────────────────────────────────────────────

const STATE_STYLE = {
  pending: {
    card:  'bg-white/80 border border-gray-200/80 shadow-sm',
    badge: 'text-gray-400 bg-gray-100/80',
    title: 'text-gray-400',
    msg:   'text-gray-400',
  },
  running: {
    card:  'bg-white border-2 border-purple-400 shadow-lg shadow-purple-200/50',
    badge: 'text-purple-600 bg-purple-100',
    title: 'text-purple-950',
    msg:   'text-purple-700',
  },
  success: {
    card:  'bg-white border border-green-300/70 shadow-sm',
    badge: 'text-green-700 bg-green-100/80',
    title: 'text-gray-900',
    msg:   'text-gray-500',
  },
  error: {
    card:  'bg-white border border-red-300 shadow-sm',
    badge: 'text-red-600 bg-red-100/80',
    title: 'text-red-800',
    msg:   'text-red-600',
  },
} as const

// ── Sub-components ────────────────────────────────────────────────────────────

function StateIcon({ state }: { state: NodeState }) {
  if (state === 'running') return <Loader2 size={14} className="text-purple-500 animate-spin flex-shrink-0" />
  if (state === 'success') return <CheckCircle size={14} className="text-green-500 flex-shrink-0" />
  if (state === 'error')   return <AlertCircle size={14} className="text-red-500 flex-shrink-0" />
  return <Circle size={14} className="text-gray-300 flex-shrink-0" />
}

interface NodeCardProps {
  step: PipelineStep
  state: NodeState
  message: string
  timestamp: string | null
  index: number
}

function NodeCard({ step, state, message, timestamp, index }: NodeCardProps) {
  const s = STATE_STYLE[state]
  return (
    <motion.div
      initial={{ opacity: 0, y: 10, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.4, delay: index * 0.05, ease: 'easeOut' }}
      className={`w-72 rounded-2xl p-4 ${s.card} transition-all duration-400`}
    >
      {/* Type badge row */}
      <div className="flex items-center justify-between mb-3">
        <div className={`flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${s.badge}`}>
          {step.icon}
          <span>{step.typeLabel}</span>
        </div>
        {timestamp && state === 'success' && (
          <span className="text-xs font-mono text-gray-400">{timestamp}</span>
        )}
        {state === 'running' && (
          <span className="text-xs font-mono text-purple-400 animate-pulse">running…</span>
        )}
      </div>

      {/* Content */}
      <div className="flex items-start gap-3">
        <div className="mt-0.5">
          <StateIcon state={state} />
        </div>
        <div className="min-w-0 flex-1">
          <div className={`text-sm font-semibold leading-tight ${s.title}`}>{step.label}</div>
          <div className={`text-xs mt-1 leading-relaxed ${s.msg} line-clamp-2`}>{message}</div>
        </div>
      </div>
    </motion.div>
  )
}

// Connector between nodes — lights up when the upper node is done
function Connector({ active, running }: { active: boolean; running: boolean }) {
  return (
    <div className="flex flex-col items-center" style={{ height: 36 }}>
      {/* Top dot */}
      <div
        className={`w-1.5 h-1.5 rounded-full mt-1 transition-colors duration-500 ${
          active ? 'bg-purple-400' : 'bg-gray-200'
        }`}
      />
      {/* Line */}
      <div className="flex-1 w-px my-1 relative overflow-hidden rounded-full">
        <div className={`absolute inset-0 transition-colors duration-700 ${active ? 'bg-purple-200' : 'bg-gray-200'}`} />
        {/* Animated travel dot */}
        {running && (
          <motion.div
            className="absolute left-0 right-0 h-3 bg-gradient-to-b from-purple-400 to-transparent rounded-full"
            animate={{ top: ['0%', '100%'] }}
            transition={{ duration: 0.8, repeat: Infinity, ease: 'linear' }}
          />
        )}
      </div>
      {/* Bottom dot */}
      <div
        className={`w-1.5 h-1.5 rounded-full mb-1 transition-colors duration-500 ${
          active ? 'bg-purple-400' : 'bg-gray-200'
        }`}
      />
    </div>
  )
}

// ── Main component ────────────────────────────────────────────────────────────

interface Props {
  logs: BackendLog[]
  isRunning: boolean
  needsApproval?: boolean
  pipelineState?: string
}

export default function PipelineGraph({ logs, isRunning, needsApproval, pipelineState }: Props) {
  const pipelineComplete = pipelineState === 'complete'
  const completedCount = STEPS.filter((s, i) => {
    const next = STEPS[i + 1]?.agent ?? null
    return getState(s.agent, logs, next, pipelineComplete) === 'success'
  }).length
  const hasError = STEPS.some((s, i) => {
    const next = STEPS[i + 1]?.agent ?? null
    return getState(s.agent, logs, next, pipelineComplete) === 'error'
  })

  return (
    <motion.div
      initial={{ opacity: 0, y: 24 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5, ease: 'easeOut' }}
      className="glass rounded-3xl p-8 shadow-xl shadow-purple-200/30 relative overflow-hidden"
    >
      {/* Dot-grid background */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          backgroundImage: 'radial-gradient(circle, #ddd6fe 1px, transparent 1px)',
          backgroundSize: '22px 22px',
          opacity: 0.45,
        }}
      />

      {/* Header */}
      <div className="relative flex items-center justify-between mb-8">
        <div>
          <h3 className="font-grotesk font-bold text-lg text-purple-950">Execution Pipeline</h3>
          <p className="text-xs text-purple-400 mt-0.5">Real-time execution graph</p>
        </div>

        <div className="flex items-center gap-2">
          {isRunning && !needsApproval && (
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-purple-100 border border-purple-200/80">
              <Loader2 size={12} className="text-purple-500 animate-spin" />
              <span className="text-xs font-semibold text-purple-600">Running</span>
            </div>
          )}
          {needsApproval && (
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-orange-100 border border-orange-300">
              <span className="w-2 h-2 rounded-full bg-orange-500 animate-pulse" />
              <span className="text-xs font-semibold text-orange-600">Awaiting Approval</span>
            </div>
          )}
          {!isRunning && completedCount === STEPS.length && !hasError && (
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-100 border border-green-200/80">
              <CheckCircle size={12} className="text-green-600" />
              <span className="text-xs font-semibold text-green-700">Complete</span>
            </div>
          )}
          {/* Step counter */}
          <div className="px-3 py-1.5 rounded-full glass border border-purple-100/60">
            <span className="text-xs font-mono text-purple-600">
              {completedCount} / {STEPS.length}
            </span>
          </div>
        </div>
      </div>

      {/* Graph — centered column of nodes */}
      <div className="relative flex flex-col items-center">
        {STEPS.map((step, i) => {
          const nextAgent   = STEPS[i + 1]?.agent ?? null
          const prevNextAgent = STEPS[i]?.agent ?? null
          // First node: show running immediately when pipeline has started (before any logs)
          const rawState    = getState(step.agent, logs, nextAgent, pipelineComplete)
          const state: NodeState = (i === 0 && rawState === 'pending' && isRunning) ? 'running' : rawState
          const prevState   = i > 0 ? getState(STEPS[i - 1].agent, logs, prevNextAgent, pipelineComplete) : 'success'
          const connActive  = prevState === 'success' || prevState === 'running'
          const connRunning = prevState === 'running'
          const msg         = getMsg(step.agent, logs, step.defaultMsg)
          const ts          = getTs(step.agent, logs)

          return (
            <div key={step.id} className="flex flex-col items-center">
              {i > 0 && <Connector active={connActive} running={connRunning} />}
              <NodeCard step={step} state={state} message={msg} timestamp={ts} index={i} />
            </div>
          )
        })}
      </div>

      {/* Progress bar at bottom */}
      <div className="relative mt-8 h-1.5 rounded-full bg-purple-100 overflow-hidden">
        <motion.div
          className="h-full rounded-full bg-gradient-to-r from-purple-500 to-indigo-500"
          initial={{ width: '0%' }}
          animate={{ width: `${(completedCount / STEPS.length) * 100}%` }}
          transition={{ duration: 0.6, ease: 'easeOut' }}
        />
      </div>
      <div className="relative mt-2 flex justify-between text-xs text-purple-400">
        <span>0%</span>
        <span>{Math.round((completedCount / STEPS.length) * 100)}% complete</span>
      </div>
    </motion.div>
  )
}
