import { useState, useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import Editor from '@monaco-editor/react'
import { CheckCircle, Clock, Hash, Play, FileText } from 'lucide-react'
import { MOCK_TRIP_MD } from '../lib/mockData'

interface Props {
  onRun?: () => void
}

export default function TripEditor({ onRun }: Props) {
  const [content, setContent] = useState(MOCK_TRIP_MD)
  const [isValid, setIsValid] = useState(true)
  const [lastSaved, setLastSaved] = useState('just now')
  const [sessionId] = useState('449cc38a-36bd-4d00-ae6a-ccfd9bbf81a7')
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (saveTimer.current) clearTimeout(saveTimer.current)
    saveTimer.current = setTimeout(() => {
      setLastSaved('just now')
      setIsValid(content.includes('trip:') && content.includes('destination:'))
    }, 800)
  }, [content])

  return (
    <div className="glass rounded-3xl overflow-hidden shadow-xl shadow-purple-200/30 flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-purple-100/60">
        <div className="flex items-center gap-3">
          <div className="flex gap-1.5">
            <div className="w-3 h-3 rounded-full bg-red-400/70" />
            <div className="w-3 h-3 rounded-full bg-yellow-400/70" />
            <div className="w-3 h-3 rounded-full bg-green-400/70" />
          </div>
          <div className="flex items-center gap-2 text-sm font-medium text-purple-700">
            <FileText size={14} />
            <span className="font-mono">trip.md</span>
          </div>
        </div>
        <button
          onClick={onRun}
          className="flex items-center gap-2 px-4 py-2 rounded-full bg-gradient-to-r from-purple-600 to-purple-500 text-white text-xs font-semibold shadow-lg shadow-purple-300/30 hover:scale-105 transition-all duration-200"
        >
          <Play size={12} />
          Run Journey
        </button>
      </div>

      {/* Monaco Editor */}
      <div className="flex-1 min-h-[380px]">
        <Editor
          height="380px"
          defaultLanguage="yaml"
          value={content}
          onChange={(v) => setContent(v ?? '')}
          theme="vs"
          options={{
            fontSize: 13,
            fontFamily: '"JetBrains Mono", "Fira Code", monospace',
            lineHeight: 22,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            padding: { top: 16, bottom: 16 },
            renderLineHighlight: 'gutter',
            lineNumbers: 'on',
            glyphMargin: false,
            folding: false,
            overviewRulerBorder: false,
            hideCursorInOverviewRuler: true,
            scrollbar: { verticalScrollbarSize: 4, horizontalScrollbarSize: 4 },
            wordWrap: 'on',
          }}
        />
      </div>

      {/* Status bar */}
      <div className="flex items-center justify-between px-5 py-2.5 border-t border-purple-100/60 bg-purple-50/40">
        <div className="flex items-center gap-4">
          <AnimatePresence mode="wait">
            <motion.div
              key={isValid ? 'valid' : 'invalid'}
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0 }}
              className={`flex items-center gap-1.5 text-xs font-medium ${isValid ? 'text-green-600' : 'text-red-500'}`}
            >
              <CheckCircle size={12} />
              <span>{isValid ? 'YAML Valid' : 'YAML Error'}</span>
            </motion.div>
          </AnimatePresence>
          <div className="flex items-center gap-1.5 text-xs text-purple-400">
            <Clock size={11} />
            <span>Saved {lastSaved}</span>
          </div>
        </div>
        <div className="flex items-center gap-1.5 text-xs text-purple-400 font-mono">
          <Hash size={11} />
          <span className="truncate max-w-36">{sessionId.slice(0, 8)}...</span>
        </div>
      </div>
    </div>
  )
}
