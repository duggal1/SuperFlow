"use client"

import * as React from "react"
import { Channel, invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Badge } from "@/components/ui/Badge"
import { Button } from "@/components/ui/Button"
import { IOSSpinner } from "@/components/shared/global-spinner"
import { useIsLight } from "@/lib/utils/theme"
import { Orb } from "./orb"
import { PocketVoicePicker, type PocketVoice } from "./pocket-voice-picker"
import { Square, Play, Download } from "lucide-react"

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

type TtsStreamEvent =
  | { event: "started"; sample_rate: number }
  | { event: "chunk"; samples: number[] }
  | { event: "finished"; duration_ms: number; first_audio_ms: number }

type TtsSynthesisSummary = {
  duration_ms: number
  first_audio_ms: number
}

const PLACEHOLDER_TEXT = "Hello! I'm your on-device voice — ready to help you get things done."

export function TtsVoiceSection() {
  const isLight = useIsLight()
  const [status, setStatus] = React.useState<TtsStatus | null>(null)
  const [progress, setProgress] = React.useState<TtsDownloadProgress | null>(null)
  const [text, setText] = React.useState(PLACEHOLDER_TEXT)
  const [isDownloading, setIsDownloading] = React.useState(false)
  const [isSynthesizing, setIsSynthesizing] = React.useState(false)
  const [hasPreview, setHasPreview] = React.useState(false)
  const [firstAudioMs, setFirstAudioMs] = React.useState<number | null>(null)
  const [isPlaying, setIsPlaying] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [voices, setVoices] = React.useState<PocketVoice[]>([])
  const [voiceId, setVoiceId] = React.useState<string>("alba")
  const audioContextRef = React.useRef<AudioContext | null>(null)
  const audioChunksRef = React.useRef<Float32Array[]>([])
  const scheduledSourcesRef = React.useRef<Set<AudioBufferSourceNode>>(new Set())
  const nextStartTimeRef = React.useRef(0)
  const streamCompleteRef = React.useRef(false)
  const playbackGenerationRef = React.useRef(0)
  const mountedRef = React.useRef(true)

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

  const stopPlayback = React.useCallback(() => {
    playbackGenerationRef.current += 1
    for (const source of scheduledSourcesRef.current) {
      try {
        source.stop()
      } catch {
        // The source may already have ended.
      }
    }
    scheduledSourcesRef.current.clear()
    nextStartTimeRef.current = 0
    if (mountedRef.current) setIsPlaying(false)
  }, [])

  React.useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      stopPlayback()
      void audioContextRef.current?.close()
      audioContextRef.current = null
    }
  }, [stopPlayback])

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
    stopPlayback()
    audioChunksRef.current = []
    setHasPreview(false)
    setFirstAudioMs(null)
    setVoiceId(id)
    try {
      await invoke("tts_set_voice", { voice: id })
    } catch (e) {
      if (!mountedRef.current) return
      setVoiceId(prev)
      setError(String(e))
    }
  }, [stopPlayback, voiceId])

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
    setHasPreview(false)
    setFirstAudioMs(null)
    stopPlayback()
    audioChunksRef.current = []
    streamCompleteRef.current = false
    try {
      const context = audioContextRef.current ?? new AudioContext({ sampleRate: 24_000 })
      audioContextRef.current = context
      await context.resume()
      nextStartTimeRef.current = context.currentTime
      const generation = playbackGenerationRef.current
      const channel = new Channel<TtsStreamEvent>()
      channel.onmessage = (event) => {
        if (!mountedRef.current || generation !== playbackGenerationRef.current) return
        if (event.event === "started") {
          setIsPlaying(true)
          return
        }
        if (event.event === "finished") {
          streamCompleteRef.current = true
          setHasPreview(true)
          setFirstAudioMs(event.first_audio_ms)
          if (scheduledSourcesRef.current.size === 0) setIsPlaying(false)
          return
        }

        const samples = Float32Array.from(event.samples, (sample) => sample / 32_768)
        audioChunksRef.current.push(samples)
        const buffer = context.createBuffer(1, samples.length, event.samples.length > 0 ? 24_000 : context.sampleRate)
        buffer.copyToChannel(samples, 0)
        const source = context.createBufferSource()
        source.buffer = buffer
        source.connect(context.destination)
        const startAt = Math.max(context.currentTime + 0.025, nextStartTimeRef.current)
        nextStartTimeRef.current = startAt + buffer.duration
        scheduledSourcesRef.current.add(source)
        source.onended = () => {
          scheduledSourcesRef.current.delete(source)
          if (
            mountedRef.current &&
            generation === playbackGenerationRef.current &&
            streamCompleteRef.current &&
            scheduledSourcesRef.current.size === 0
          ) {
            setIsPlaying(false)
          }
        }
        source.start(startAt)
      }
      const summary = await invoke<TtsSynthesisSummary>("tts_synthesize", {
        text: trimmed,
        onEvent: channel,
      })
      if (mountedRef.current) {
        setHasPreview(true)
        setFirstAudioMs(summary.first_audio_ms)
      }
    } catch (e) {
      stopPlayback()
      if (mountedRef.current) setError(String(e))
    } finally {
      if (mountedRef.current) setIsSynthesizing(false)
    }
  }, [stopPlayback, text])

  const togglePlay = React.useCallback(() => {
    if (isPlaying) {
      stopPlayback()
      return
    }
    const context = audioContextRef.current
    const chunks = audioChunksRef.current
    if (!context || chunks.length === 0) return
    stopPlayback()
    const generation = playbackGenerationRef.current
    const sampleCount = chunks.reduce((total, chunk) => total + chunk.length, 0)
    const samples = new Float32Array(sampleCount)
    let offset = 0
    for (const chunk of chunks) {
      samples.set(chunk, offset)
      offset += chunk.length
    }
    const buffer = context.createBuffer(1, samples.length, 24_000)
    buffer.copyToChannel(samples, 0)
    const source = context.createBufferSource()
    source.buffer = buffer
    source.connect(context.destination)
    scheduledSourcesRef.current.add(source)
    source.onended = () => {
      scheduledSourcesRef.current.delete(source)
      if (mountedRef.current && generation === playbackGenerationRef.current) setIsPlaying(false)
    }
    setIsPlaying(true)
    void context.resume().then(() => source.start())
  }, [isPlaying, stopPlayback])

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
            Download the on-device model and eight prepared voices to get started.
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
              icon={isSynthesizing ? <IOSSpinner size={12} /> : <Play size={14} />}
            >
              {isSynthesizing ? "Streaming…" : hasPreview ? "Regenerate" : "Play"}
            </Button>
            {hasPreview && (
              <Button
                variant="secondary"
                size="sm"
                onClick={togglePlay}
                icon={isPlaying ? <Square size={14} /> : <Play size={14} />}
              >
                {isPlaying ? "Stop" : "Play preview"}
              </Button>
            )}
            <span className={`ml-auto text-[11px] ${isLight ? "text-stone-400" : "text-stone-500"}`}>
              {firstAudioMs === null ? "Metal · 24 kHz" : `First audio · ${firstAudioMs} ms`}
            </span>
          </div>

          {hasPreview && (
            <div className={`flex items-center gap-3 rounded-lg px-3 py-2.5 ${isLight ? "bg-stone-50" : "bg-[#32302d]"}`}>
              <button
                type="button"
                onClick={togglePlay}
                className={`flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-full ${isLight ? "bg-blue-600 text-white hover:bg-blue-700" : "bg-blue-600 text-white hover:bg-blue-700"}`}
                aria-label={isPlaying ? "Stop" : "Play"}
              >
                {isPlaying ? <Square className="size-3.5" /> : <Play className="size-4" />}
              </button>
              <div className="min-w-0 flex-1">
                <p className={`truncate text-xs font-medium ${isLight ? "text-stone-900" : "text-stone-100"}`}>Voice preview</p>
                <p className={`text-[11px] ${isLight ? "text-stone-500" : "text-stone-400"}`}>On-device · Streaming PCM</p>
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
