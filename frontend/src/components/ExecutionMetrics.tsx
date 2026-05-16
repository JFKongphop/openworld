import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import { CheckCircle, Package, DollarSign, Database, Cpu, Hash } from 'lucide-react'

interface Metric {
  label: string
  value: number
  target: number
  unit: string
  icon: React.ReactNode
  color: string
  glow: string
}

export default function ExecutionMetrics() {
  const [counts, setCounts] = useState([0, 0, 0, 0, 0, 0])

  const metrics: Metric[] = [
    { label: 'Agents Completed', value: 5, target: 5, unit: '', icon: <Cpu size={18} />, color: 'from-purple-500 to-purple-600', glow: 'shadow-purple-300/40' },
    { label: 'Segments Reserved', value: 7, target: 7, unit: '', icon: <CheckCircle size={18} />, color: 'from-indigo-500 to-indigo-600', glow: 'shadow-indigo-300/40' },
    { label: 'Budget Used', value: 985, target: 1500, unit: 'USD', icon: <DollarSign size={18} />, color: 'from-emerald-500 to-emerald-600', glow: 'shadow-emerald-300/40' },
    { label: 'Root Hashes Stored', value: 2, target: 2, unit: '', icon: <Database size={18} />, color: 'from-pink-500 to-pink-600', glow: 'shadow-pink-300/40' },
    { label: 'NFTs Minted', value: 1, target: 1, unit: '', icon: <Package size={18} />, color: 'from-violet-500 to-violet-600', glow: 'shadow-violet-300/40' },
    { label: 'Txs Confirmed', value: 3, target: 3, unit: '', icon: <Hash size={18} />, color: 'from-sky-500 to-sky-600', glow: 'shadow-sky-300/40' },
  ]

  // Count-up animation
  useEffect(() => {
    metrics.forEach((m, i) => {
      let current = 0
      const step = Math.ceil(m.value / 30)
      const id = setInterval(() => {
        current = Math.min(current + step, m.value)
        setCounts((prev) => {
          const next = [...prev]
          next[i] = current
          return next
        })
        if (current >= m.value) clearInterval(id)
      }, 40 + i * 10)
    })
  }, [])

  return (
    <section className="px-6 py-16">
      <div className="max-w-7xl mx-auto">
        <div className="mb-10">
          <h2 className="font-grotesk text-3xl font-bold text-purple-950">Execution Metrics</h2>
          <p className="text-purple-500 mt-1">Realtime orchestration telemetry</p>
        </div>

        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
          {metrics.map((m, i) => (
            <motion.div
              key={m.label}
              initial={{ opacity: 0, y: 24 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: i * 0.08, duration: 0.5 }}
              className={`glass rounded-2xl p-5 shadow-lg ${m.glow} hover:scale-105 transition-transform duration-200`}
            >
              <div className={`w-10 h-10 rounded-xl bg-gradient-to-br ${m.color} flex items-center justify-center text-white mb-4 shadow-md`}>
                {m.icon}
              </div>
              <div className="text-3xl font-bold font-grotesk gradient-text">
                {counts[i]}{m.unit && <span className="text-sm font-medium ml-1">{m.unit}</span>}
              </div>
              <div className="text-xs text-purple-500 mt-1 leading-tight">{m.label}</div>

              {/* Progress bar */}
              {m.target > 1 && (
                <div className="mt-3 h-1 rounded-full bg-purple-100 overflow-hidden">
                  <motion.div
                    initial={{ width: 0 }}
                    whileInView={{ width: `${(m.value / m.target) * 100}%` }}
                    viewport={{ once: true }}
                    transition={{ delay: i * 0.08 + 0.3, duration: 0.8 }}
                    className={`h-full rounded-full bg-gradient-to-r ${m.color}`}
                  />
                </div>
              )}
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  )
}
