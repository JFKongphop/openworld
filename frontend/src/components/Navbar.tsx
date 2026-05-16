import { Globe, Zap } from 'lucide-react'

export default function Navbar() {
  return (
    <nav className="fixed top-0 left-0 right-0 z-50 px-6 py-3">
      <div className="max-w-7xl mx-auto glass rounded-2xl px-6 py-3 flex items-center justify-between">
        {/* Logo */}
        <div className="flex items-center gap-3">
          <div className="relative">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-purple-500 to-purple-700 flex items-center justify-center shadow-lg">
              <Globe size={18} className="text-white" />
            </div>
            <div className="absolute -inset-1 rounded-xl bg-purple-500 opacity-20 blur-sm" />
          </div>
          <div>
            <span className="font-grotesk font-bold text-lg text-purple-900 tracking-tight">OpenWorld</span>
            <span className="ml-2 text-xs text-purple-400 font-medium hidden sm:inline">AUTONOMOUS TRAVEL</span>
          </div>
        </div>

        {/* Center Nav */}
        <div className="hidden md:flex items-center gap-1">
          {['Dashboard', 'Artifacts', 'trip.md', 'Root Hashes', 'Explorer'].map((item) => (
            <a
              key={item}
              href={`#${item.toLowerCase().replace('.', '').replace(' ', '-')}`}
              className="px-4 py-2 text-sm font-medium text-purple-700 hover:text-purple-900 hover:bg-purple-50/60 rounded-lg transition-all duration-200"
            >
              {item}
            </a>
          ))}
        </div>

        {/* Right */}
        <div className="flex items-center gap-3">
          <div className="hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-purple-50/80 border border-purple-200/50">
            <span className="w-2 h-2 rounded-full bg-green-500" />
            <span className="text-xs font-medium text-purple-700">0G Galileo</span>
          </div>
          <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg glass-dark text-xs font-mono text-purple-700">
            0x874604...24ae
          </div>
          <button className="flex items-center gap-2 px-4 py-2 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white text-sm font-semibold shadow-lg shadow-purple-300/30 hover:shadow-purple-400/40 hover:scale-105 transition-all duration-200">
            <Zap size={14} />
            <span>New Journey</span>
          </button>
        </div>
      </div>
    </nav>
  )
}
