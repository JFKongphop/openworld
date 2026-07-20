import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { MapPin, Clock, DollarSign, CheckCircle, ChevronDown, ChevronUp, Copy, ExternalLink } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { BackendArtifact } from '../hooks/useSession'

const DESTINATION_EMOJI: Record<string, string> = {
  Tokyo: '🗾', Japan: '🗾',
  Paris: '🗼', France: '🗼',
  London: '🎡', UK: '🎡',
  Singapore: '🦁',
  Dubai: '🏙️',
  Bangkok: '🏯', Thailand: '🏯',
  Bali: '🌴', Indonesia: '🌴',
  New: '🗽', York: '🗽',
  default: '✈️',
}

function getEmoji(destination: string) {
  const word = destination.split(/[\s,]+/)[0]
  return DESTINATION_EMOJI[word] ?? DESTINATION_EMOJI.default
}

interface Props {
  artifact: BackendArtifact
  reportMd?: string | null
}

export default function TripResult({ artifact, reportMd }: Props) {
  const [showReport, setShowReport] = useState(false)
  const [copied, setCopied] = useState(false)

  const emoji = getEmoji(artifact.destination)

  const copy = (text: string) => {
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 32 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5, ease: 'easeOut' }}
      className="glass rounded-3xl overflow-hidden shadow-2xl shadow-purple-300/30"
    >
      {/* Banner */}
      <div className="relative h-36 bg-gradient-to-br from-purple-600 via-purple-700 to-indigo-700 overflow-hidden">
        <div
          className="absolute inset-0 opacity-10"
          style={{
            backgroundImage:
              'linear-gradient(rgba(255,255,255,0.15) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.15) 1px, transparent 1px)',
            backgroundSize: '28px 28px',
          }}
        />
        {/* Success badge */}
        <div className="absolute top-4 right-5 flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-green-500/90 text-white text-xs font-bold shadow">
          <CheckCircle size={12} />
          Pipeline Complete
        </div>
        {/* Destination */}
        <div className="absolute bottom-4 left-6 flex items-end gap-4">
          <span className="text-6xl leading-none">{emoji}</span>
          <div>
            <div className="text-white font-grotesk font-bold text-3xl leading-tight">{artifact.destination}</div>
            <div className="text-purple-200 text-sm mt-0.5">AI-generated itinerary</div>
          </div>
        </div>
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-3 divide-x divide-purple-100/60 border-b border-purple-100/60">
        {[
          { icon: <Clock size={16} />, label: 'Duration', value: `${artifact.duration_days} days` },
          { icon: <DollarSign size={16} />, label: 'Total Spent', value: `$${artifact.total_spent_usd.toLocaleString()} USD` },
          { icon: <MapPin size={16} />, label: 'Destination', value: artifact.destination },
        ].map(({ icon, label, value }) => (
          <div key={label} className="px-6 py-4 flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-purple-50 border border-purple-100 flex items-center justify-center text-purple-500 flex-shrink-0">
              {icon}
            </div>
            <div>
              <div className="text-xs text-purple-400">{label}</div>
              <div className="text-sm font-bold text-purple-900">{value}</div>
            </div>
          </div>
        ))}
      </div>

      {/* Trip summary */}
      <div className="px-6 py-5">
        <div className="text-xs font-bold text-purple-400 uppercase tracking-widest mb-3">AI Summary</div>
        <p className="text-sm text-purple-800 leading-relaxed">{artifact.trip_summary}</p>
      </div>

      {/* Artifact ID row */}
      <div className="px-6 pb-4 flex items-center justify-between gap-4">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xs text-purple-400 flex-shrink-0">Artifact ID</span>
          <span className="text-xs font-mono text-purple-600 truncate">{artifact.artifact_id}</span>
          <button
            onClick={() => copy(artifact.artifact_id)}
            className="flex-shrink-0 p-1 rounded hover:bg-purple-100 transition-colors"
          >
            {copied ? <span className="text-xs text-green-600">✓</span> : <Copy size={11} className="text-purple-400" />}
          </button>
        </div>

        {reportMd && (
          <button
            onClick={() => setShowReport((v) => !v)}
            className="flex items-center gap-1.5 px-4 py-2 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white text-xs font-semibold shadow hover:scale-105 transition-transform flex-shrink-0"
          >
            <ExternalLink size={12} />
            {showReport ? 'Hide Report' : 'View Full Report'}
            {showReport ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>
        )}
      </div>

      {/* Expandable report */}
      <AnimatePresence>
        {showReport && reportMd && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3 }}
            className="overflow-hidden border-t border-purple-100/60"
          >
            <div className="px-6 py-5 max-h-[480px] overflow-y-auto">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ children }) => <h1 className="text-xl font-bold text-purple-950 mt-5 mb-2 font-grotesk">{children}</h1>,
                  h2: ({ children }) => <h2 className="text-lg font-bold text-purple-950 mt-6 mb-2 font-grotesk border-b border-purple-100 pb-1.5">{children}</h2>,
                  h3: ({ children }) => <h3 className="text-base font-semibold text-purple-900 mt-4 mb-1">{children}</h3>,
                  p:  ({ children }) => <p  className="text-sm text-purple-800 leading-relaxed mb-3">{children}</p>,
                  ul: ({ children }) => <ul className="mb-3 space-y-1 pl-1">{children}</ul>,
                  ol: ({ children }) => <ol className="mb-3 space-y-1 pl-4 list-decimal">{children}</ol>,
                  li: ({ children }) => (
                    <li className="flex gap-2 text-sm text-purple-800 leading-relaxed">
                      <span className="text-purple-400 mt-0.5 flex-shrink-0">•</span>
                      <span>{children}</span>
                    </li>
                  ),
                  strong: ({ children }) => <strong className="font-semibold text-purple-900">{children}</strong>,
                  em:     ({ children }) => <em className="italic text-purple-600">{children}</em>,
                  code:   ({ children }) => <code className="text-xs bg-purple-50 border border-purple-100 rounded px-1.5 py-0.5 font-mono text-purple-700">{children}</code>,
                  pre:    ({ children }) => <pre className="bg-purple-50 border border-purple-100 rounded-xl p-4 overflow-x-auto text-xs font-mono text-purple-800 mb-3">{children}</pre>,
                  blockquote: ({ children }) => <blockquote className="border-l-4 border-purple-300 pl-4 my-3 text-sm text-purple-600 italic bg-purple-50/40 py-2 rounded-r-lg">{children}</blockquote>,
                  hr: () => <hr className="my-5 border-purple-100" />,
                  // ── GFM tables ────────────────────────────────────────────
                  table: ({ children }) => (
                    <div className="overflow-x-auto mb-4 rounded-xl border border-purple-100 shadow-sm">
                      <table className="w-full text-sm border-collapse">{children}</table>
                    </div>
                  ),
                  thead: ({ children }) => <thead className="bg-purple-50">{children}</thead>,
                  tbody: ({ children }) => <tbody className="divide-y divide-purple-50">{children}</tbody>,
                  tr:    ({ children }) => <tr className="hover:bg-purple-50/40 transition-colors">{children}</tr>,
                  th:    ({ children }) => <th className="px-4 py-2.5 text-left text-xs font-bold text-purple-700 uppercase tracking-wide whitespace-nowrap">{children}</th>,
                  td:    ({ children }) => <td className="px-4 py-2.5 text-sm text-purple-800">{children}</td>,
                }}
              >
                {reportMd}
              </ReactMarkdown>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}
