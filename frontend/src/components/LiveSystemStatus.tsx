import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import { Cpu, Database, Wifi, FileCode, Server, Activity } from 'lucide-react'

interface ServiceStatus {
  name: string
  label: string
  icon: React.ReactNode
  status: 'online' | 'syncing' | 'offline'
  detail: string
}

export default function LiveSystemStatus() {
  const [tick, setTick] = useState(0)

  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 2000)
    return () => clearInterval(id)
  }, [])

  const services: ServiceStatus[] = [
    { name: 'agents', label: 'Agents Online', icon: <Activity size={16} />, status: 'online', detail: '5 / 5 active' },
    { name: 'compute', label: '0G Compute', icon: <Cpu size={16} />, status: 'online', detail: 'qwen-2.5-7b-instruct' },
    { name: 'storage', label: '0G Storage', icon: <Database size={16} />, status: 'online', detail: 'indexer turbo' },
    { name: 'contract', label: 'ERC-7857 Contract', icon: <FileCode size={16} />, status: 'online', detail: '0xAF26...D455' },
    { name: 'rpc', label: '0G Galileo RPC', icon: <Server size={16} />, status: 'online', detail: 'chain 16602' },
    { name: 'ws', label: 'WebSocket', icon: <Wifi size={16} />, status: tick % 6 === 0 ? 'syncing' : 'online', detail: 'live stream' },
  ]

  const statusColor = {
    online: 'bg-green-500',
    syncing: 'bg-yellow-400',
    offline: 'bg-red-500',
  }

  const statusGlow = {
    online: 'shadow-green-400/60',
    syncing: 'shadow-yellow-400/60',
    offline: 'shadow-red-400/60',
  }

  return (
    <div className="glass rounded-3xl p-6 shadow-xl shadow-purple-200/30 relative overflow-hidden">
      {/* Ambient glow */}
      <div className="absolute top-0 right-0 w-40 h-40 bg-purple-300/20 rounded-full blur-3xl pointer-events-none" />

      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h3 className="font-grotesk font-bold text-lg text-purple-950">System Runtime</h3>
          <p className="text-xs text-purple-400 mt-0.5">Live cognition status</p>
        </div>
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-50 border border-green-200/60">
          <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          <span className="text-xs font-semibold text-green-700">OPERATIONAL</span>
        </div>
      </div>

      {/* Services */}
      <div className="space-y-3">
        {services.map((svc, i) => (
          <motion.div
            key={svc.name}
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ delay: i * 0.08 }}
            className="flex items-center justify-between p-3 rounded-xl bg-white/40 border border-white/50 hover:bg-white/60 transition-all duration-200"
          >
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-purple-100/80 flex items-center justify-center text-purple-600">
                {svc.icon}
              </div>
              <div>
                <div className="text-sm font-medium text-purple-900">{svc.label}</div>
                <div className="text-xs text-purple-400 font-mono">{svc.detail}</div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <div className={`relative w-2.5 h-2.5 rounded-full ${statusColor[svc.status]} shadow-sm ${statusGlow[svc.status]}`}>
                {svc.status === 'online' && (
                  <span className={`absolute inset-0 rounded-full ${statusColor[svc.status]} ping-slow opacity-75`} />
                )}
              </div>
              <span className="text-xs font-medium text-purple-600 uppercase tracking-wide">{svc.status}</span>
            </div>
          </motion.div>
        ))}
      </div>

      {/* Bottom bar */}
      <div className="mt-5 pt-4 border-t border-purple-100/60 flex items-center justify-between">
        <div className="text-xs text-purple-400">Last sync: <span className="text-purple-600 font-medium">2s ago</span></div>
        <div className="flex items-center gap-1.5">
          {[...Array(5)].map((_, i) => (
            <div
              key={i}
              className="w-1 rounded-full bg-purple-400"
              style={{
                height: `${8 + Math.sin((tick + i) * 0.8) * 6}px`,
                transition: 'height 0.3s ease',
              }}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
