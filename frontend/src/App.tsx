import { useState } from 'react'
import { motion } from 'framer-motion'
import Navbar from './components/Navbar'
import HeroSection from './components/HeroSection'
import TripEditor from './components/TripEditor'
import ActivityFeed from './components/ActivityFeed'
import ExecutionMetrics from './components/ExecutionMetrics'
import RootHashExplorer from './components/RootHashExplorer'
import TxFeed from './components/TxFeed'
import ArtifactGallery from './components/ArtifactGallery'
import Footer from './components/Footer'
import { useChainData } from './hooks/useChainData'

export default function App() {
  const [runKey, setRunKey] = useState(0)
  const { rootHashes, txEvents, artifacts, loading } = useChainData()

  return (
    <div className="min-h-screen bg-[#F8F5FF] grid-bg relative">
      {/* Global ambient blobs */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden">
        <div className="absolute top-0 left-1/3 w-[500px] h-[500px] bg-purple-300/12 rounded-full blur-3xl" />
        <div className="absolute top-1/2 right-0 w-96 h-96 bg-purple-400/10 rounded-full blur-3xl" />
        <div className="absolute bottom-0 left-0 w-80 h-80 bg-indigo-300/10 rounded-full blur-3xl" />
      </div>

      <Navbar />

      <main className="relative">
        {/* Hero */}
        <HeroSection />

        {/* Editor + Activity Feed */}
        <section id="tripmded" className="px-6 py-16">
          <div className="max-w-7xl mx-auto">
            <div className="mb-10">
              <h2 className="font-grotesk text-3xl font-bold text-purple-950">Trip Policy Editor</h2>
              <p className="text-purple-500 mt-1">Programme your autonomous travel agent with trip.md</p>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 h-[520px]">
              <TripEditor onRun={() => setRunKey((k) => k + 1)} />
              <ActivityFeed key={runKey} />
            </div>
          </div>
        </section>

        {/* Metrics */}
        <ExecutionMetrics />

        {/* Root Hash Explorer */}
        <RootHashExplorer rows={rootHashes} loading={loading} />

        {/* TX Feed */}
        <TxFeed txs={txEvents} loading={loading} />

        {/* Artifact Gallery */}
        <ArtifactGallery artifacts={artifacts} loading={loading} />

        {/* CTA Banner */}
        <section className="px-6 py-16">
          <div className="max-w-4xl mx-auto">
            <motion.div
              initial={{ opacity: 0, y: 24 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              className="glass rounded-3xl p-12 text-center relative overflow-hidden shadow-2xl shadow-purple-300/20"
            >
              <div className="absolute inset-0 bg-gradient-to-br from-purple-500/5 to-indigo-500/5" />
              <div className="absolute top-0 left-1/2 -translate-x-1/2 w-64 h-32 bg-purple-400/15 rounded-full blur-3xl" />
              <div className="relative">
                <div className="text-4xl mb-4">✈️</div>
                <h2 className="font-grotesk text-4xl font-bold text-purple-950 mb-4">
                  Start Your <span className="gradient-text">Autonomous Journey</span>
                </h2>
                <p className="text-purple-600 mb-8 max-w-lg mx-auto">
                  Edit trip.md, hit run. AI agents handle everything — flights, hotels, activities — and persist the result on-chain.
                </p>
                <div className="flex justify-center gap-4">
                  <a
                    href="#tripmded"
                    className="px-8 py-3.5 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white font-semibold shadow-xl shadow-purple-300/40 hover:shadow-purple-400/50 hover:scale-105 transition-all duration-200"
                  >
                    Open Editor
                  </a>
                  <a
                    href="https://scan-testnet.0g.ai/address/0xAF2699e9d306b57F5541aE3f04C43586589fD455"
                    target="_blank"
                    rel="noreferrer"
                    className="px-8 py-3.5 rounded-full glass border border-purple-200/60 text-purple-700 font-semibold hover:bg-purple-50 transition-all duration-200"
                  >
                    View Contract
                  </a>
                </div>
              </div>
            </motion.div>
          </div>
        </section>
      </main>

      <Footer />
    </div>
  )
}
