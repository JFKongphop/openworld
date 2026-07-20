import { useState } from 'react'
import { motion } from 'framer-motion'
import Navbar from './components/Navbar'
import HeroSection from './components/HeroSection'
import TripEditor from './components/TripEditor'
import ActivityFeed from './components/ActivityFeed'
import TripResult from './components/TripResult'
import PipelineGraph from './components/PipelineGraph'
import ApprovalGate from './components/ApprovalGate'
import Footer from './components/Footer'
import AgentShowcase from './components/AgentShowcase'
import { useSession } from './hooks/useSession'

export default function App() {
  const [runKey, setRunKey] = useState(0)
  const { logs, pipelineState, isRunning, needsApproval, artifact, reportMd, runPipeline, approve, reject } = useSession()

  const handleRun = (travelMd: string) => {
    setRunKey((k) => k + 1)
    runPipeline(travelMd)
  }

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

        {/* 7-Agent Showcase */}
        <AgentShowcase />

        {/* Editor + Activity Feed */}
        <section id="tripmded" className="px-6 py-16">
          <div className="max-w-7xl mx-auto">
            <div className="mb-10">
              <h2 className="font-grotesk text-3xl font-bold text-purple-950">Workflow Policy Editor</h2>
              <p className="text-purple-500 mt-1">Define your travel workflow in YAML. OpenWorld converts your policy into an autonomous execution plan.</p>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 h-[520px]">
              <TripEditor onRun={handleRun} />
              <ActivityFeed logs={logs} isRunning={isRunning} />
            </div>
          </div>
        </section>

        {/* Trip Result — appears after pipeline completes */}
        {artifact && (
          <section className="px-6 pb-4">
            <div className="max-w-7xl mx-auto">
              <TripResult artifact={artifact} reportMd={reportMd} />
            </div>
          </section>
        )}

        {/* Pipeline Graph — slides in when pipeline starts, left=graph right=feed */}
        {(isRunning || needsApproval || logs.length > 0) && (
          <section className="px-6 pb-12" id="pipeline">
            <div className="max-w-7xl mx-auto">
              <div className="mb-8">
                <h2 className="font-grotesk text-3xl font-bold text-purple-950">Autonomous Workflow Execution</h2>
                <p className="text-purple-500 mt-1">Live agent pipeline — each node activates as the agent runs</p>
              </div>

              {/* Approval gate — shown as prominent banner when awaiting human decision */}
              {needsApproval && (
                <div className="mb-6">
                  <ApprovalGate logs={logs} onApprove={approve} onReject={reject} />
                </div>
              )}

              <div className="grid grid-cols-1 lg:grid-cols-[1fr_360px] gap-6 items-start">
                <PipelineGraph logs={logs} isRunning={isRunning} needsApproval={needsApproval} pipelineState={pipelineState} />
                <div className="sticky top-24 h-[640px]">
                  <ActivityFeed key={`feed-${runKey}`} logs={logs} isRunning={isRunning} needsApproval={needsApproval} />
                </div>
              </div>
            </div>
          </section>
        )}

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
                  Edit trip.md, hit run. AI agents handle everything — flights, hotels, activities — and save the full itinerary to cloud storage.
                </p>
                <div className="flex justify-center gap-4">
                  <a
                    href="#tripmded"
                    className="px-8 py-3.5 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white font-semibold shadow-xl shadow-purple-300/40 hover:shadow-purple-400/50 hover:scale-105 transition-all duration-200"
                  >
                    Open Editor
                  </a>
                  <a
                    href="https://github.com/openworld-travel"
                    target="_blank"
                    rel="noreferrer"
                    className="px-8 py-3.5 rounded-full glass border border-purple-200/60 text-purple-700 font-semibold hover:bg-purple-50 transition-all duration-200"
                  >
                    View on GitHub
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
