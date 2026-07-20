import { motion } from 'framer-motion'
import { ArrowRight, Globe, Cpu, Database } from 'lucide-react'
import LiveSystemStatus from './LiveSystemStatus'

export default function HeroSection() {
  return (
    <section className="relative min-h-screen flex items-center pt-24 pb-16 px-6 overflow-hidden">
      {/* Background blobs */}
      <div className="absolute inset-0 pointer-events-none">
        <div className="absolute top-20 left-1/4 w-96 h-96 bg-purple-300/20 rounded-full blur-3xl" />
        <div className="absolute bottom-20 right-1/4 w-80 h-80 bg-purple-400/15 rounded-full blur-3xl" />
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[400px] bg-purple-200/10 rounded-full blur-3xl" />
      </div>

      <div className="relative max-w-7xl mx-auto w-full grid grid-cols-1 lg:grid-cols-2 gap-16 items-center">
        {/* Left — Text */}
        <div>
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-full glass border border-purple-200/60 mb-8"
          >
            <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
            <span className="text-xs font-semibold text-purple-700 tracking-widest uppercase">
              AUTONOMOUS AI OPERATOR
            </span>
          </motion.div>

          <motion.h1
            initial={{ opacity: 0, y: 24 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.1 }}
            className="font-grotesk text-6xl lg:text-7xl font-bold leading-tight text-purple-950 mb-6"
          >
            Autonomous{' '}
            <span className="gradient-text">Travel</span>
            <br />
            Operations
          </motion.h1>

          <motion.p
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.2 }}
            className="text-lg text-purple-700/80 leading-relaxed mb-10 max-w-xl"
          >
            OpenWorld automates travel workflows end-to-end. AI agents understand your request, research options, execute reservations, recover from failures, and ask for approval before critical decisions.
          </motion.p>

          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.3 }}
            className="flex flex-wrap gap-4"
          >
            <a
              href="#tripmded"
              className="flex items-center gap-2 px-7 py-3.5 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white font-semibold shadow-xl shadow-purple-300/40 hover:shadow-purple-400/50 hover:scale-105 transition-all duration-200"
            >
              Launch Workflow
              <ArrowRight size={16} />
            </a>
            <a
              href="#artifacts"
              className="flex items-center gap-2 px-7 py-3.5 rounded-full glass border border-purple-200/60 text-purple-700 font-semibold hover:bg-purple-50/60 transition-all duration-200"
            >
              View Executions
            </a>
          </motion.div>

          {/* Stats row */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.8, delay: 0.5 }}
            className="flex gap-10 mt-14"
          >
            {[
              { icon: Globe, label: 'Workflows Completed', value: '1' },
              { icon: Cpu, label: 'Tasks Automated', value: '24+' },
              { icon: Database, label: 'Recovery Success Rate', value: '2' },
            ].map(({ icon: Icon, label, value }) => (
              <div key={label}>
                <div className="text-2xl font-bold font-grotesk gradient-text">{value}</div>
                <div className="text-xs text-purple-500 mt-0.5">{label}</div>
              </div>
            ))}
          </motion.div>
        </div>

        {/* Right — Live System Status */}
        <motion.div
          initial={{ opacity: 0, x: 30 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.8, delay: 0.2 }}
        >
          <LiveSystemStatus />
        </motion.div>
      </div>
    </section>
  )
}
