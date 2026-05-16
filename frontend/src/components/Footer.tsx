import { Globe, Github, ExternalLink } from 'lucide-react'

export default function Footer() {
  return (
    <footer className="px-6 py-12 border-t border-purple-100/60">
      <div className="max-w-7xl mx-auto">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-10 mb-10">
          {/* Brand */}
          <div className="md:col-span-2">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-purple-500 to-purple-700 flex items-center justify-center shadow">
                <Globe size={18} className="text-white" />
              </div>
              <span className="font-grotesk font-bold text-lg text-purple-900">OpenWorld</span>
            </div>
            <p className="text-sm text-purple-500 leading-relaxed max-w-xs">
              Programmable autonomous travel infrastructure.
              AI agents orchestrate real-world journeys and persist them on-chain as ERC-7857 NFTs.
            </p>
            <div className="flex gap-3 mt-5">
              <a
                href="https://github.com"
                target="_blank"
                rel="noreferrer"
                className="p-2 rounded-lg glass border border-purple-100/60 hover:bg-purple-50 transition-colors"
              >
                <Github size={16} className="text-purple-600" />
              </a>
              <a
                href="https://0g.ai"
                target="_blank"
                rel="noreferrer"
                className="p-2 rounded-lg glass border border-purple-100/60 hover:bg-purple-50 transition-colors"
              >
                <ExternalLink size={16} className="text-purple-600" />
              </a>
            </div>
          </div>

          {/* Links */}
          <div>
            <div className="text-xs font-bold text-purple-400 uppercase tracking-widest mb-4">Infrastructure</div>
            <ul className="space-y-2">
              {[
                { label: '0G Compute', href: 'https://0g.ai' },
                { label: '0G Storage', href: 'https://0g.ai' },
                { label: '0G Galileo Testnet', href: 'https://scan-testnet.0g.ai' },
                { label: 'ERC-7857 Contract', href: 'https://scan-testnet.0g.ai/address/0xAF2699e9d306b57F5541aE3f04C43586589fD455' },
              ].map(({ label, href }) => (
                <li key={label}>
                  <a href={href} target="_blank" rel="noreferrer" className="text-sm text-purple-600 hover:text-purple-800 transition-colors">
                    {label}
                  </a>
                </li>
              ))}
            </ul>
          </div>

          <div>
            <div className="text-xs font-bold text-purple-400 uppercase tracking-widest mb-4">Agents</div>
            <ul className="space-y-2">
              {['PlannerAgent', 'SearchAgent', 'ReservationAgent', 'VaultAgent', 'ArtifactAgent'].map((a) => (
                <li key={a}>
                  <span className="text-sm text-purple-600">{a}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>

        <div className="pt-6 border-t border-purple-100/60 flex flex-col sm:flex-row items-center justify-between gap-4">
          <p className="text-xs text-purple-400">
            © 2026 OpenWorld. Built on <span className="text-purple-600 font-medium">0G Network</span>.
          </p>
          <div className="flex items-center gap-4 text-xs text-purple-400">
            <span className="font-mono">Chain: 16602</span>
            <span className="font-mono">Contract: 0xAF26...D455</span>
          </div>
        </div>
      </div>
    </footer>
  )
}
