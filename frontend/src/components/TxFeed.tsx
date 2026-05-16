import { useState, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { ExternalLink, Zap, Loader2 } from 'lucide-react'
import type { TxEvent } from '../lib/mockData'

interface Props {
  txs: TxEvent[]
  loading?: boolean
}

const STATUS_STYLE: Record<TxEvent['status'], { dot: string; text: string; bg: string }> = {
  success: { dot: 'bg-green-500',  text: 'text-green-700',  bg: 'bg-green-50 border-green-200' },
  pending: { dot: 'bg-yellow-400 animate-pulse', text: 'text-yellow-700', bg: 'bg-yellow-50 border-yellow-200' },
  failed:  { dot: 'bg-red-500',    text: 'text-red-700',    bg: 'bg-red-50 border-red-200' },
}

export default function TxFeed({ txs, loading }: Props) {
  const [list, setList] = useState<TxEvent[]>(txs)

  useEffect(() => { setList(txs) }, [txs])

  return (
    <section className="px-6 py-16">
      <div className="max-w-7xl mx-auto">
        <div className="mb-10 flex items-center justify-between">
          <div>
            <h2 className="font-grotesk text-3xl font-bold text-purple-950">On-Chain Transactions</h2>
            <p className="text-purple-500 mt-1">Live blockchain interactions — 0G Galileo testnet</p>
          </div>
          <div className="flex items-center gap-2 text-xs font-semibold text-green-600">
            <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
            LIVE
          </div>
        </div>

        <div className="glass rounded-3xl overflow-hidden shadow-xl shadow-purple-200/20">
          {loading ? (
            <div className="flex items-center justify-center gap-3 py-16 text-purple-400">
              <Loader2 size={20} className="animate-spin" />
              <span className="text-sm">Fetching from 0G Galileo…</span>
            </div>
          ) : (
          <AnimatePresence initial={false}>
            {list.map((tx) => {
              const s = STATUS_STYLE[tx.status]
              return (
                <motion.div
                  key={tx.id}
                  initial={{ opacity: 0, y: -16 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.35 }}
                  className="flex items-center justify-between px-6 py-4 border-b border-purple-50/80 hover:bg-white/50 transition-colors duration-150"
                >
                  {/* Left */}
                  <div className="flex items-center gap-4">
                    <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-purple-500 to-purple-600 flex items-center justify-center shadow">
                      <Zap size={16} className="text-white" />
                    </div>
                    <div>
                      <div className="text-sm font-semibold text-purple-900">{tx.action}</div>
                      <div className="text-xs font-mono text-purple-400 mt-0.5">{tx.hash}</div>
                    </div>
                  </div>

                  {/* Center */}
                  <div className="hidden md:flex flex-col items-center">
                    <div className="text-xs text-purple-400">Block</div>
                    <div className="text-sm font-mono text-purple-700">{tx.block.toLocaleString()}</div>
                  </div>

                  {/* Right */}
                  <div className="flex items-center gap-3">
                    <span className="text-xs text-purple-400">{tx.timestamp}</span>
                    <span className={`inline-flex items-center gap-1.5 text-xs font-semibold px-2.5 py-1 rounded-full border ${s.bg} ${s.text}`}>
                      <span className={`w-1.5 h-1.5 rounded-full ${s.dot}`} />
                      {tx.status.toUpperCase()}
                    </span>
                    <a
                      href={`https://scan-testnet.0g.ai/tx/${tx.hash}`}
                      target="_blank"
                      rel="noreferrer"
                      className="p-1.5 rounded-lg hover:bg-purple-100 transition-colors"
                    >
                      <ExternalLink size={13} className="text-purple-400" />
                    </a>
                  </div>
                </motion.div>
              )
            })}
          </AnimatePresence>
          )}
        </div>
      </div>
    </section>
  )
}
