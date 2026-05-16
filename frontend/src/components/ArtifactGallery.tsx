import { motion } from 'framer-motion'
import { ExternalLink, Copy, MapPin, Calendar, DollarSign, Hash, Loader2 } from 'lucide-react'
import { useState } from 'react'
import type { JourneyArtifact } from '../lib/mockData'

interface Props {
  artifacts: JourneyArtifact[]
  loading?: boolean
}

const DESTINATION_EMOJI: Record<string, string> = {
  Tokyo: '🗾',
  Paris: '🗼',
  London: '🎡',
  Singapore: '🦁',
  Dubai: '🏙️',
  default: '✈️',
}

function ArtifactCard({ artifact }: { artifact: JourneyArtifact }) {
  const [copied, setCopied] = useState(false)
  const emoji = DESTINATION_EMOJI[artifact.destination] ?? DESTINATION_EMOJI.default

  const copy = (text: string) => {
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 24 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      whileHover={{ y: -4 }}
      transition={{ duration: 0.4 }}
      className="glass rounded-3xl overflow-hidden shadow-xl shadow-purple-200/30 flex flex-col"
    >
      {/* Card banner */}
      <div className="relative h-32 bg-gradient-to-br from-purple-500 via-purple-600 to-indigo-600 overflow-hidden">
        <div className="absolute inset-0 opacity-20"
          style={{
            backgroundImage: 'linear-gradient(rgba(255,255,255,0.1) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.1) 1px, transparent 1px)',
            backgroundSize: '24px 24px',
          }}
        />
        <div className="absolute bottom-3 left-5 flex items-end gap-3">
          <span className="text-5xl">{emoji}</span>
          <div>
            <div className="text-white font-grotesk font-bold text-xl leading-tight">{artifact.destination}</div>
            <div className="text-purple-200 text-xs">from {artifact.origin}</div>
          </div>
        </div>
        <div className="absolute top-3 right-4 px-2.5 py-1 rounded-full bg-white/20 border border-white/30 text-white text-xs font-semibold">
          #{artifact.tokenId} ERC-7857
        </div>
      </div>

      {/* Body */}
      <div className="p-5 flex-1 flex flex-col gap-4">
        <div className="grid grid-cols-2 gap-3">
          {[
            { icon: <Calendar size={13} />, label: 'Dates', value: artifact.dates },
            { icon: <DollarSign size={13} />, label: 'Spent', value: artifact.totalSpent },
            { icon: <Hash size={13} />, label: 'Segments', value: `${artifact.segments} bookings` },
            { icon: <MapPin size={13} />, label: 'Session', value: artifact.sessionId.slice(0, 8) + '…' },
          ].map(({ icon, label, value }) => (
            <div key={label} className="p-2.5 rounded-xl bg-white/50 border border-white/60">
              <div className="flex items-center gap-1.5 text-purple-400 mb-1">
                {icon}
                <span className="text-xs">{label}</span>
              </div>
              <div className="text-sm font-semibold text-purple-900 truncate">{value}</div>
            </div>
          ))}
        </div>

        {/* Report hash */}
        <div className="p-3 rounded-xl bg-purple-50/60 border border-purple-100">
          <div className="text-xs text-purple-400 mb-1">Report Root Hash</div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs font-mono text-purple-700 truncate">{artifact.reportHash.slice(0, 22)}…</span>
            <button
              onClick={() => copy(artifact.reportHash)}
              className="flex-shrink-0 p-1 rounded hover:bg-purple-100 transition-colors"
            >
              {copied ? <span className="text-xs text-green-600">✓</span> : <Copy size={12} className="text-purple-400" />}
            </button>
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-2 mt-auto">
          <a
            href={`https://scan-testnet.0g.ai/tx/${artifact.txHash}`}
            target="_blank"
            rel="noreferrer"
            className="flex-1 flex items-center justify-center gap-1.5 py-2.5 rounded-xl bg-gradient-to-r from-purple-600 to-purple-500 text-white text-xs font-semibold shadow hover:shadow-purple-300/50 hover:scale-105 transition-all duration-200"
          >
            <ExternalLink size={12} />
            View on Explorer
          </a>
          <button
            onClick={() => copy(artifact.txHash)}
            className="px-3 py-2.5 rounded-xl glass border border-purple-200/60 text-purple-600 text-xs font-semibold hover:bg-purple-50 transition-colors"
          >
            <Copy size={12} />
          </button>
        </div>
      </div>
    </motion.div>
  )
}

export default function ArtifactGallery({ artifacts, loading }: Props) {
  return (
    <section id="artifacts" className="px-6 py-16">
      <div className="max-w-7xl mx-auto">
        <div className="mb-10">
          <h2 className="font-grotesk text-3xl font-bold text-purple-950">Journey Artifacts</h2>
          <p className="text-purple-500 mt-1">ERC-7857 intelligent NFTs — autonomous travel memories on-chain</p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {loading ? (
            <div className="col-span-3 flex items-center justify-center gap-3 py-20 text-purple-400">
              <Loader2 size={24} className="animate-spin" />
              <span className="text-sm">Loading journey artifacts from chain…</span>
            </div>
          ) : artifacts.map((a) => (
            <ArtifactCard key={a.id} artifact={a} />
          ))}

          {/* Placeholder card */}
          <motion.div
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true }}
            className="glass rounded-3xl border-2 border-dashed border-purple-200/60 flex flex-col items-center justify-center p-10 min-h-64 text-center"
          >
            <div className="w-12 h-12 rounded-2xl bg-purple-100 flex items-center justify-center mb-4">
              <span className="text-2xl">✈️</span>
            </div>
            <div className="text-purple-700 font-semibold mb-1">Your next journey</div>
            <div className="text-sm text-purple-400">Run a trip to mint your ERC-7857 artifact</div>
          </motion.div>
        </div>
      </div>
    </section>
  )
}
