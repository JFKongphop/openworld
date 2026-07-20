import { motion } from 'framer-motion'
import {
  Brain,
  Search,
  CreditCard,
  ShieldCheck,
  RefreshCw,
  Archive,
  FileText,
  FileCode,
} from 'lucide-react'

const AGENTS = [
  {
    step: '0',
    name: 'Intent',
    role: 'Policy Parsing',
    description: 'Parses your trip.md file and extracts structured intent, constraints, and budget rules.',
    icon: FileCode,
    gradient: 'from-fuchsia-500 to-purple-500',
    glow: 'shadow-fuchsia-400/30',
    badge: 'bg-fuchsia-100 text-fuchsia-700',
  },
  {
    step: '1',
    name: 'Planner Agent',
    role: 'Itinerary Design',
    description: 'Generates a structured travel plan from your trip.md policy.',
    icon: Brain,
    gradient: 'from-violet-500 to-purple-600',
    glow: 'shadow-violet-400/30',
    badge: 'bg-violet-100 text-violet-700',
  },
  {
    step: '2',
    name: 'Search Agent',
    role: 'Research & Tool Invocation',
    description: 'Searches flights, hotels, weather, maps, and local knowledge using external tools.',
    icon: Search,
    gradient: 'from-blue-500 to-indigo-600',
    glow: 'shadow-blue-400/30',
    badge: 'bg-blue-100 text-blue-700',
  },
  {
    step: '3',
    name: 'Reservation Agent',
    role: 'Autonomous Booking',
    description: 'Executes reservations and automates browser or API interactions.',
    icon: CreditCard,
    gradient: 'from-indigo-500 to-blue-600',
    glow: 'shadow-indigo-400/30',
    badge: 'bg-indigo-100 text-indigo-700',
  },
  {
    step: '4',
    name: 'Approval Agent',
    role: 'Human-in-the-loop',
    description: 'Requests human approval before payments or other critical actions.',
    icon: ShieldCheck,
    gradient: 'from-emerald-500 to-teal-600',
    glow: 'shadow-emerald-400/30',
    badge: 'bg-emerald-100 text-emerald-700',
  },
  {
    step: '5',
    name: 'Recovery Agent',
    role: 'Failure Handling',
    description: 'Detects errors and retries with alternative strategies automatically.',
    icon: RefreshCw,
    gradient: 'from-orange-500 to-amber-500',
    glow: 'shadow-orange-400/30',
    badge: 'bg-orange-100 text-orange-700',
  },
  {
    step: '6',
    name: 'Memory Agent',
    role: 'Session Context',
    description: 'Stores user preferences, previous trips and execution history to improve future decisions.',
    icon: Archive,
    gradient: 'from-pink-500 to-rose-500',
    glow: 'shadow-pink-400/30',
    badge: 'bg-pink-100 text-pink-700',
  },
  {
    step: '7',
    name: 'Report Agent',
    role: 'Report Generation',
    description: 'Generates execution reports, workflow logs, and reusable travel artifacts.',
    icon: FileText,
    gradient: 'from-sky-500 to-cyan-500',
    glow: 'shadow-sky-400/30',
    badge: 'bg-sky-100 text-sky-700',
  },
]

export default function AgentShowcase() {
  return (
    <section className="px-6 py-20">
      <div className="max-w-7xl mx-auto">
        {/* Section header */}
        <div className="text-center mb-12">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-purple-100 text-purple-600 text-xs font-semibold uppercase tracking-widest mb-4">
            <span className="w-1.5 h-1.5 rounded-full bg-purple-500 animate-pulse" />
            8 Autonomous Agents
          </div>
          <h2 className="font-grotesk text-4xl font-bold text-purple-950 mb-3">
            How OpenWorld Executes Your Workflow
          </h2>
          <p className="text-purple-500 text-lg max-w-2xl mx-auto">
            Specialized AI agents collaborate to understand requests, invoke external tools, recover from failures, and request human approval before critical actions.
          </p>
        </div>

        {/* Agent cards row */}
        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-8 gap-4">
          {AGENTS.map((agent, i) => {
            const Icon = agent.icon
            return (
              <motion.div
                key={agent.step}
                initial={{ opacity: 0, y: 24 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true }}
                transition={{ delay: i * 0.07, duration: 0.4 }}
                className="group relative flex flex-col items-center text-center p-4 rounded-2xl bg-white border border-purple-100 hover:border-purple-300 hover:shadow-lg transition-all duration-200"
              >
                {/* Step badge */}
                <span className={`absolute top-3 right-3 text-[10px] font-bold px-1.5 py-0.5 rounded-md ${agent.badge}`}>
                  {agent.step}
                </span>

                {/* Icon */}
                <div
                  className={`w-12 h-12 rounded-xl bg-gradient-to-br ${agent.gradient} flex items-center justify-center shadow-lg ${agent.glow} mb-3 group-hover:scale-110 transition-transform duration-200`}
                >
                  <Icon className="w-5 h-5 text-white" strokeWidth={2} />
                </div>

                {/* Name */}
                <p className="font-grotesk text-sm font-bold text-purple-950 mb-0.5">{agent.name}</p>
                <p className={`text-[10px] font-semibold mb-2 ${agent.badge.split(' ')[1]}`}>{agent.role}</p>

                {/* Description — hidden on small breakpoints for cleanliness */}
                <p className="hidden lg:block text-[11px] text-purple-400 leading-relaxed">{agent.description}</p>


              </motion.div>
            )
          })}
        </div>
      </div>
    </section>
  )
}
