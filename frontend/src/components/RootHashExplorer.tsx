import { useState } from 'react'
import { motion } from 'framer-motion'
import { Copy, ExternalLink, Database, FileText, HardDrive, ScrollText, Loader2 } from 'lucide-react'
import type { RootHash } from '../lib/mockData'

interface Props {
  rows: RootHash[]
  loading?: boolean
}

const TYPE_STYLES: Record<RootHash['type'], { label: string; color: string; icon: React.ReactNode }> = {
  REPORT:   { label: 'REPORT',   color: 'text-purple-700 bg-purple-100 border-purple-200', icon: <FileText size={12} /> },
  MEMORY:   { label: 'MEMORY',   color: 'text-indigo-700 bg-indigo-100 border-indigo-200', icon: <Database size={12} /> },
  ARTIFACT: { label: 'ARTIFACT', color: 'text-pink-700 bg-pink-100 border-pink-200',       icon: <HardDrive size={12} /> },
  LOG:      { label: 'LOG',      color: 'text-gray-700 bg-gray-100 border-gray-200',        icon: <ScrollText size={12} /> },
}

function truncateHash(h: string, start = 10, end = 6) {
  return `${h.slice(0, start)}...${h.slice(-end)}`
}

export default function RootHashExplorer({ rows, loading }: Props) {
  const [copied, setCopied] = useState<string | null>(null)

  const handleCopy = (text: string, id: string) => {
    navigator.clipboard.writeText(text)
    setCopied(id)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <section id="root-hashes" className="px-6 py-16">
      <div className="max-w-7xl mx-auto">
        <div className="mb-10">
          <h2 className="font-grotesk text-3xl font-bold text-purple-950">Root Hash Explorer</h2>
          <p className="text-purple-500 mt-1">Decentralised memory on 0G Storage</p>
        </div>

        <div className="glass rounded-3xl overflow-hidden shadow-xl shadow-purple-200/20">
          {/* Table header */}
          <div className="grid grid-cols-12 gap-4 px-6 py-3 border-b border-purple-100/60 bg-purple-50/40">
            <div className="col-span-4 text-xs font-semibold text-purple-500 uppercase tracking-wider">File</div>
            <div className="col-span-4 text-xs font-semibold text-purple-500 uppercase tracking-wider">Root Hash</div>
            <div className="col-span-1 text-xs font-semibold text-purple-500 uppercase tracking-wider">Type</div>
            <div className="col-span-1 text-xs font-semibold text-purple-500 uppercase tracking-wider">Size</div>
            <div className="col-span-1 text-xs font-semibold text-purple-500 uppercase tracking-wider">When</div>
            <div className="col-span-1 text-xs font-semibold text-purple-500 uppercase tracking-wider">Actions</div>
          </div>

          {/* Rows */}
          {loading ? (
            <div className="flex items-center justify-center gap-3 py-16 text-purple-400">
              <Loader2 size={20} className="animate-spin" />
              <span className="text-sm">Fetching from 0G Galileo…</span>
            </div>
          ) : rows.map((row, i) => {
            const typeInfo = TYPE_STYLES[row.type]
            return (
              <motion.div
                key={row.id}
                initial={{ opacity: 0, y: 8 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ delay: i * 0.1 }}
                className="grid grid-cols-12 gap-4 px-6 py-4 border-b border-purple-50/80 hover:bg-white/50 transition-colors duration-150 items-center group"
              >
                {/* File */}
                <div className="col-span-4">
                  <div className="text-sm font-medium text-purple-900 truncate">{row.filename}</div>
                  <div className="text-xs text-purple-400 font-mono mt-0.5 truncate">{truncateHash(row.txHash)}</div>
                </div>

                {/* Hash */}
                <div className="col-span-4">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-mono text-purple-700 truncate">{truncateHash(row.hash)}</span>
                    <button
                      onClick={() => handleCopy(row.hash, row.id + '-hash')}
                      className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded hover:bg-purple-100"
                    >
                      {copied === row.id + '-hash'
                        ? <span className="text-xs text-green-600">✓</span>
                        : <Copy size={12} className="text-purple-400" />
                      }
                    </button>
                  </div>
                </div>

                {/* Type */}
                <div className="col-span-1">
                  <span className={`inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-full border ${typeInfo.color}`}>
                    {typeInfo.icon}
                    {typeInfo.label}
                  </span>
                </div>

                {/* Size */}
                <div className="col-span-1 text-sm text-purple-600">{row.size}</div>

                {/* When */}
                <div className="col-span-1 text-sm text-purple-400">{row.uploadedAgo}</div>

                {/* Actions */}
                <div className="col-span-1 flex items-center gap-1">
                  <button
                    onClick={() => handleCopy(row.hash, row.id + '-copy')}
                    className="p-1.5 rounded-lg hover:bg-purple-100 transition-colors"
                    title="Copy hash"
                  >
                    {copied === row.id + '-copy'
                      ? <span className="text-xs text-green-600">✓</span>
                      : <Copy size={13} className="text-purple-400" />
                    }
                  </button>
                  <a
                    href={`https://scan-testnet.0g.ai/tx/${row.txHash}`}
                    target="_blank"
                    rel="noreferrer"
                    className="p-1.5 rounded-lg hover:bg-purple-100 transition-colors"
                    title="View on explorer"
                  >
                    <ExternalLink size={13} className="text-purple-400" />
                  </a>
                </div>
              </motion.div>
            )
          })}

          {/* Empty state */}
          {!loading && rows.length === 0 && (
            <div className="px-6 py-16 text-center text-purple-400">
              <Database size={32} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">No root hashes yet — run a journey to upload files to 0G Storage</p>
            </div>
          )}
        </div>
      </div>
    </section>
  )
}
