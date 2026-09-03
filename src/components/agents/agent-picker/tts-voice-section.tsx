"use client"

import * as React from "react"
import { invoke } from "@tauri-apps/api/core"
import { convertFileSrc } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Badge } from "@/components/ui/Badge"
import { Button } from "@/components/ui/Button"
import { IOSSpinner } from "@/components/shared/global-spinner"
import { useIsLight } from "@/lib/utils/theme"
import { Orb } from "./orb"
import { PocketVoicePicker, type PocketVoice } from "./pocket-voice-picker"
import { Pause, Play, Download } from "lucide-react"

type TtsStatus = {
  engine_available: boolean
  model_downloaded: boolean
  model_size_bytes: number
  downloading: boolean
}

type TtsDownloadProgress = {
  downloaded_bytes: number
  total_bytes: number
  percent: number
}

const PLACEHOLDER_TEXT = "Hello! I'm your on-device voice — ready to help you get things done."

export function TtsVoiceSection() {
  const isLight = useIsLight()
  const [status, setStatus] = React.useState<TtsStatus | null>(null)
  const [progress, setProgress] = React.useState<TtsDownloadProgress | null>(null)
  const [text, setText] = React.useState(PLACEHOLDER_TEXT)
  const [isDownloading, setIsDownloading] = React.useState(false)
  const [isSynthesizing, setIsSynthesizing] = React.useState(false)
  const [audioSrc, setAudioSrc] = React.useState<string | null>(null)
  const [isPlaying, setIsPlaying] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [voices, setVoices] = React.useState<PocketVoice[]>([])
  const [voiceId, setVoiceId] = React.useState<string>("alba")
  const audioRef = React.useRef<HTMLAudioElement | null>(null)
  const mountedRef = React.useRef(true)
  const synthTimerRef = React.useRef<number | null>(null)

  const orbColors = React.useMemo<[string, string]>(
    () => ["#BCCFF7", "#144FFF"],
    [],
  )

  const refreshStatus = React.useCallback(async () => {
    try {
      const s = await invoke<TtsStatus>("tts_status")
      if (!mountedRef.current) return
      setStatus(s)
      if (s.model_downloaded) setIsDownloading(false)
    } catch (e) {
      if (!mountedRef.current) return
      console.error("tts_status failed", e)
    }
  }, [])

  React.useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      if (synthTimerRef.current !== null) {
        window.clearTimeout(synthTimerRef.current)
        synthTimerRef.current = null
      }
      if (audioRef.current) {
        audioRef.current.pause()
        audioRef.current.removeAttribute("src")
      }
    }
  }, [])

  const refreshVoices = React.useCallback(async () => {
    try {
      const list = await invoke<PocketVoice[]>("tts_voices")
      if (!mountedRef.current) return
      setVoices(list)
      const selected = await invoke<string>("tts_selected_voice")
      if (!mountedRef.current) return
      if (list.some((v) => v.id === selected)) setVoiceId(selected)
    } catch (e) {
      if (!mountedRef.current) return
      console.error("tts_voices failed", e)
    }
  }, [])

  const handleVoiceChange = React.useCallback(async (id: string) => {
    if (!mountedRef.current) return
    const prev = voiceId
    setVoiceId(id)
    try {
      await invoke("tts_set_voice", { voice: id })
    } catch (e) {
      if (!mountedRef.current) return
      setVoiceId(prev)
      setError(String(e))
    }
  }, [voiceId])

  React.useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined
    void refreshStatus()
    void refreshVoices()
    const p = listen<TtsDownloadProgress>("tts-download-progress", (event) => {
      if (cancelled || !mountedRef.current) return
      setProgress(event.payload)
      if (event.payload.percent >= 100) {
        setIsDownloading(false)
        void refreshStatus()
      }
    }).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
      return fn
    })
    p.catch((e) => console.error("tts-download-progress listen failed", e))
    return () => {
      cancelled = true
      if (unlisten) unlisten()
      else void p.then((fn) => fn()).catch(() => {})
    }
  }, [refreshStatus, refreshVoices])

  const handleDownload = React.useCallback(async () => {
    if (!mountedRef.current) return
    setError(null)
    setIsDownloading(true)
    setProgress({ downloaded_bytes: 0, total_bytes: 0, percent: 0 })
    try {
      await invoke("tts_download_model")
      if (mountedRef.current) await refreshStatus()
    } catch (e) {
      if (mountedRef.current) {
        setError(String(e))
        setIsDownloading(false)
      }
    }
  }, [refreshStatus])

  const handleSynthesize = React.useCallback(async () => {
    const trimmed = text.trim()
    if (!trimmed) {
      if (mountedRef.current) setError("Enter some text to synthesize")
      return
    }
    if (trimmed.length > 2000) {
      if (mountedRef.current) setError("Text too long (max 2000 chars)")
      return
    }
    if (!mountedRef.current) return
    setError(null)
    setIsSynthesizing(true)
    try {
      const filePath = await invoke<string>("tts_synthesize", { text: trimmed })
      if (!mountedRef.current) return
      const assetUrl = convertFileSrc(filePath, "asset")
      setAudioSrc(assetUrl)
      if (synthTimerRef.current !== null) window.clearTimeout(synthTimerRef.current)
      synthTimerRef.current = window.setTimeout(() => {
        synthTimerRef.current = null
        if (!mountedRef.current || !audioRef.current) return
        audioRef.current.src = assetUrl
        audioRef.current.load()
        void audioRef.current.play().catch(() => {})
      }, 50)
    } catch (e) {
      if (mountedRef.current) setError(String(e))
    } finally {
      if (mountedRef.current) setIsSynthesizing(false)
    }
  }, [text])

  const togglePlay = React.useCallback(() => {
    if (!audioRef.current || !audioSrc) return
    if (isPlaying) {
      audioRef.current.pause()
    } else {
      void audioRef.current.play().catch(() => {})
    }
  }, [isPlaying, audioSrc])

  const isDownloaded = status?.model_downloaded === true
  const showProgress = isDownloading && progress !== null

  return (
    <section
      className={`flex flex-col gap-4 rounded-xl px-5 py-5 ${
        isLight ? "border border-stone-200/70 bg-white" : "bg-stone-800"
      }`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-transparent">
            <div className="relative size-8 overflow-visible">
              <Orb colors={orbColors} agentState={isPlaying ? "talking" : isDownloaded ? "thinking" : null} className="absolute inset-0" />
            </div>
          </div>
          <p className={`text-[15px] font-normal tracking-tight ${isLight ? "text-stone-900" : "text-stone-100"}`}>Pocket TTS · On-device voice</p>
        </div>
      </div>

      <div className="flex items-center justify-between gap-3 pt-1">
        <div className="flex items-center gap-2">
          {!isDownloaded && (
            <Button
              variant="secondary"
              size="sm"
              onClick={handleDownload}
              disabled={isDownloading}
              icon={isDownloading ? <IOSSpinner size={12} /> : <Download size={14} />}
            >
              {isDownloading ? "Downloading…" : "Download model"}
            </Button>
          )}
        </div>
        <div className="flex items-center gap-2">
          {status === null ? (
            <IOSSpinner size={14} />
          ) : isDownloaded ? (
            <Badge variant="green">
              <span className="size-1.5 rounded-full bg-green-500" />
              Active
            </Badge>
          ) : (
            <Badge variant="rose">Not downloaded</Badge>
          )}
        </div>
      </div>

      {!isDownloaded ? (
        <div className="flex flex-col gap-3 pt-1">
          <p className={`text-xs leading-4 ${isLight ? "text-stone-600" : "text-stone-400"}`}>
            Download this to get started. Only 200 MB at BF16 with 8 voices included.
          </p>
          {showProgress && (
            <div className={`flex flex-col gap-1.5 rounded-lg px-3 py-2.5 ${isLight ? "bg-stone-50" : "bg-[#32302d]"}`}>
              <div className="flex items-center justify-between">
                <span className={`text-xs ${isLight ? "text-stone-700" : "text-stone-200"}`}>Downloading… {progress.percent}%</span>
                <span className={`text-[11px] tabular-nums ${isLight ? "text-stone-500" : "text-stone-400"}`}>
                  {(progress.downloaded_bytes / (1024 * 1024)).toFixed(1)} / {(progress.total_bytes / (1024 * 1024)).toFixed(1)} MB
                </span>
              </div>
              <div className={`h-1.5 w-full overflow-hidden rounded-full ${isLight ? "bg-stone-200" : "bg-stone-700"}`}>
                <div
                  className="h-full bg-blue-600 transition-all duration-150"
                  style={{ width: `${progress.percent}%` }}
                />
              </div>
            </div>
          )}
          {error && <p className="text-xs text-rose-500">{error}</p>}
          {!status?.engine_available && (
            <div className="flex items-center gap-2">
              <span className={`text-[11px] ${isLight ? "text-amber-600" : "text-amber-500"}`}>Engine not bundled</span>
            </div>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-4 pt-1">
          <PocketVoicePicker voices={voices} value={voiceId} onValueChange={(id) => void handleVoiceChange(id)} />

          <div className="flex flex-col gap-2">
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={3}
              maxLength={2000}
              placeholder={PLACEHOLDER_TEXT}
              className={`w-full resize-none rounded-lg border px-3 py-2.5 text-sm leading-5 outline-none transition-colors placeholder:text-stone-400 ${
                isLight
                  ? "border-stone-200 bg-white text-stone-900 focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  : "border-white/[0.06] bg-[#32302d] text-stone-100 focus:border-blue-600"
              }`}
            />
            <div className="flex items-center justify-between">
              <span className={`text-[11px] tabular-nums ${isLight ? "text-stone-400" : "text-stone-500"}`}>{text.length}/2000</span>
              {error && <span className="text-xs text-rose-500">{error}</span>}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleSynthesize}
              disabled={isSynthesizing || !text.trim()}
              icon={isSynthesizing ? <IOSSpinner size={12} /> : isPlaying ? <Pause size={14} /> : <Play size={14} />}
            >
              {isSynthesizing ? "Synthesizing…" : audioSrc ? "Regenerate" : "Play"}
            </Button>
            {audioSrc && (
              <Button
                variant="secondary"
                size="sm"
                onClick={togglePlay}
                icon={isPlaying ? <Pause size={14} /> : <Play size={14} />}
              >
                {isPlaying ? "Pause" : "Play preview"}
              </Button>
            )}
            <span className={`ml-auto text-[11px] ${isLight ? "text-stone-400" : "text-stone-500"}`}>CPU · 24 kHz</span>
          </div>

          <audio
            ref={audioRef}
            className="hidden"
            crossOrigin="anonymous"
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onEnded={() => setIsPlaying(false)}
            onError={() => setError("Audio playback failed")}
          />

          {audioSrc && (
            <div className={`flex items-center gap-3 rounded-lg px-3 py-2.5 ${isLight ? "bg-stone-50" : "bg-[#32302d]"}`}>
              <button
                type="button"
                onClick={togglePlay}
                className={`flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full ${isLight ? "bg-blue-600 text-white hover:bg-blue-700" : "bg-blue-600 text-white hover:bg-blue-700"}`}
                aria-label={isPlaying ? "Pause" : "Play"}
              >
                {isPlaying ? <Pause className="size-4" /> : <Play className="size-4" />}
              </button>
              <div className="min-w-0 flex-1">
                <p className={`truncate text-xs font-medium ${isLight ? "text-stone-900" : "text-stone-100"}`}>preview.wav</p>
                <p className={`text-[11px] ${isLight ? "text-stone-500" : "text-stone-400"}`}>On-device · Pocket TTS</p>
              </div>
              <span className={`shrink-0 text-[11px] ${isPlaying ? "text-blue-600" : isLight ? "text-stone-400" : "text-stone-500"}`}>
                {isPlaying ? "Playing" : "Ready"}
              </span>
            </div>
          )}
        </div>
      )}
    </section>
  )
}
