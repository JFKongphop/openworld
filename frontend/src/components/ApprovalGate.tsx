import { motion } from 'framer-motion'
import { ShieldAlert, CheckCircle, XCircle, DollarSign, AlertTriangle } from 'lucide-react'
import type { BackendLog } from '../hooks/useSession'

interface Props {
  logs: BackendLog[]
  onApprove: () => void
  onReject: () => void
}

/** Extract the last VaultAgent log matching a pattern */
function findVaultMsg(logs: BackendLog[], keyword: string): string | null {
  const matches = logs.filter(
    l => l.agent === 'VaultAgent' && l.message.toLowerCase().includes(keyword.toLowerCase())
  )
  return matches.length ? matches[matches.length - 1].message : null
}

function parseBudget(msg: string | null): { spent: string; total: string; pct: string } {
  if (!msg) return { spent: '—', total: '—', pct: '—' }
  // "86% of budget committed (1288 / 1500 USD)"
  const pctMatch  = msg.match(/(\d+)%/)
  const numMatch  = msg.match(/\((\d[\d,]+)\s*\/\s*(\d[\d,]+)/)
  return {
    pct:   pctMatch  ? `${pctMatch[1]}%`  : '—',
    spent: numMatch  ? `$${numMatch[1]}`  : '—',
    total: numMatch  ? `$${numMatch[2]}`  : '—',
  }
}

export default function ApprovalGate({ logs, onApprove, onReject }: Props) {
  const gateMsg    = findVaultMsg(logs, 'Approval gate triggered') ?? findVaultMsg(logs, 'awaiting_approval')
  const summaryMsg = findVaultMsg(logs, 'Vault summary')
  const pauseMsg   = findVaultMsg(logs, 'Pipeline paused')
  const budget     = parseBudget(gateMsg ?? pauseMsg)

  // Collect the last few VaultAgent messages for context
  const vaultLogs  = logs.filter(l => l.agent === 'VaultAgent').slice(-5)

  return (
    <motion.div
      initial={{ opacity: 0, y: -20, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.45, ease: 'easeOut' }}
      className="rounded-3xl overflow-hidden shadow-2xl shadow-orange-200/50 border-2 border-orange-300"
    >
      {/* Alert banner */}
      <div className="bg-gradient-to-r from-orange-500 to-amber-500 px-6 py-4 flex items-center gap-3">
        <div className="w-9 h-9 rounded-xl bg-white/20 flex items-center justify-center flex-shrink-0">
          <ShieldAlert size={20} className="text-white" />
        </div>
        <div>
          <div className="text-white font-grotesk font-bold text-lg leading-tight">
            Human Approval Required
          </div>
          <div className="text-orange-100 text-xs mt-0.5">
            VaultAgent has paused the pipeline — budget threshold reached
          </div>
        </div>
        <div className="ml-auto flex items-center gap-2 px-3 py-1.5 rounded-full bg-white/20 border border-white/30">
          <AlertTriangle size={12} className="text-white" />
          <span className="text-white text-xs font-bold">AWAITING APPROVAL</span>
        </div>
      </div>

      <div className="bg-white px-6 py-5">
        {/* Budget stats */}
        <div className="grid grid-cols-3 gap-4 mb-5">
          {[
            { label: 'Budget Committed', value: budget.spent, icon: <DollarSign size={15} className="text-orange-500" /> },
            { label: 'Total Budget',     value: budget.total, icon: <DollarSign size={15} className="text-purple-500" /> },
            { label: 'Usage Rate',       value: budget.pct,   icon: <AlertTriangle size={15} className="text-amber-500" /> },
          ].map(({ label, value, icon }) => (
            <div key={label} className="p-3 rounded-2xl bg-orange-50/60 border border-orange-100">
              <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
                {icon}
                <span>{label}</span>
              </div>
              <div className="text-xl font-bold font-grotesk text-gray-900">{value}</div>
            </div>
          ))}
        </div>

        {/* Budget bar */}
        <div className="mb-5">
          <div className="flex justify-between text-xs text-gray-500 mb-1.5">
            <span>Budget utilisation</span>
            <span className="font-semibold text-orange-600">{budget.pct}</span>
          </div>
          <div className="h-2.5 rounded-full bg-gray-100 overflow-hidden">
            <motion.div
              className="h-full rounded-full bg-gradient-to-r from-orange-400 to-amber-400"
              initial={{ width: '0%' }}
              animate={{ width: budget.pct !== '—' ? budget.pct : '86%' }}
              transition={{ duration: 0.8, ease: 'easeOut' }}
            />
          </div>
        </div>

        {/* Context logs */}
        {vaultLogs.length > 0 && (
          <div className="mb-5 p-4 rounded-2xl bg-gray-50 border border-gray-100 space-y-1.5 max-h-32 overflow-y-auto">
            <div className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Vault Log</div>
            {vaultLogs.map((log, i) => (
              <div key={i} className="flex items-start gap-2 text-xs">
                <span className="font-mono text-gray-400 flex-shrink-0">{log.timestamp}</span>
                <span className="text-gray-700">{log.message}</span>
              </div>
            ))}
          </div>
        )}

        {/* Summary message */}
        {summaryMsg && (
          <div className="mb-5 p-3 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800 font-medium">
            {summaryMsg}
          </div>
        )}

        {/* Question */}
        <p className="text-sm text-gray-700 mb-5">
          The VaultAgent has committed <strong>{budget.pct}</strong> of the travel budget ({budget.spent} / {budget.total}).
          Do you want to <strong>approve</strong> the remaining reservations and continue,
          or <strong>reject</strong> and cancel the pipeline?
        </p>

        {/* Action buttons */}
        <div className="flex gap-3">
          <button
            onClick={onApprove}
            className="flex-1 flex items-center justify-center gap-2 py-3.5 rounded-2xl bg-gradient-to-r from-green-500 to-emerald-500 text-white font-bold shadow-lg shadow-green-200/60 hover:scale-105 hover:shadow-green-300/60 transition-all duration-200"
          >
            <CheckCircle size={18} />
            Approve &amp; Continue
          </button>
          <button
            onClick={onReject}
            className="flex-1 flex items-center justify-center gap-2 py-3.5 rounded-2xl bg-white border-2 border-red-300 text-red-600 font-bold hover:bg-red-50 hover:scale-105 transition-all duration-200"
          >
            <XCircle size={18} />
            Reject &amp; Cancel
          </button>
        </div>
      </div>
    </motion.div>
  )
}
