"use client"

import * as React from "react"
import type { ElevenLabs } from "@elevenlabs/elevenlabs-js"
import { Check, ChevronsUpDown, Pause, Play } from "lucide-react"
import { Orb } from "./orb"
import { cn } from "@/lib/utils"
import { useIsLight } from "@/lib/utils/theme"
import { AudioPlayerProvider, useAudioPlayer } from "./player"

interface VoicePickerProps {
  voices: ElevenLabs.Voice[]
  value?: string
  onValueChange?: (value: string) => void
  placeholder?: string
  className?: string
  open?: boolean
  onOpenChange?: (open: boolean) => void
}

function VoicePicker({
  voices,
  value,
  onValueChange,
  placeholder = "Select a voice...",
  className,
  open,
  onOpenChange,
}: VoicePickerProps) {
  const [internalOpen, setInternalOpen] = React.useState(false)
  const [search, setSearch] = React.useState("")
  const isControlled = open !== undefined
  const isOpen = isControlled ? open : internalOpen
  const setIsOpen = isControlled ? onOpenChange : setInternalOpen
  const isLight = useIsLight()
  const containerRef = React.useRef<HTMLDivElement>(null)

  const selectedVoice = voices.find((v) => v.voiceId === value)
  const orbColors: [string, string] = ["#000e2e", "#004cff"]

  const filtered = React.useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) return voices
    return voices.filter((v) => {
      const hay = [
        v.name,
        v.labels?.accent,
        v.labels?.gender,
        v.labels?.age,
        v.labels?.description,
        v.labels?.["use case"],
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
      return hay.includes(q)
    })
  }, [voices, search])

  React.useEffect(() => {
    if (!isOpen) return
    const onDown = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setIsOpen?.(false)
    }
    document.addEventListener("mousedown", onDown)
    return () => document.removeEventListener("mousedown", onDown)
  }, [isOpen, setIsOpen])

  React.useEffect(() => {
    if (!isOpen) setSearch("")
  }, [isOpen])

  return (
    <AudioPlayerProvider>
      <div ref={containerRef} className={cn("relative w-full", className)}>
        <button
          type="button"
          role="combobox"
          aria-expanded={isOpen}
          onClick={() => setIsOpen?.(!isOpen)}
          className={cn(
            "flex h-8 w-full cursor-pointer items-center justify-between gap-2 rounded-[10px] border px-3 py-1 text-[13px] outline-none transition-colors",
            isLight
              ? "border-stone-200 bg-white text-stone-900 hover:bg-stone-50"
              : "border-white/[0.06] bg-[#363230] text-stone-50 hover:bg-white/[0.08]",
          )}
        >
          {selectedVoice ? (
            <div className="flex items-center gap-2 overflow-hidden">
              <div className="relative size-6 shrink-0 overflow-visible">
                <Orb colors={orbColors} agentState="thinking" className="absolute inset-0" />
              </div>
              <span className="truncate">{selectedVoice.name}</span>
            </div>
          ) : (
            <span className={isLight ? "text-stone-500" : "text-stone-400"}>{placeholder}</span>
          )}
          <ChevronsUpDown className="ml-2 size-4 shrink-0 opacity-50" />
        </button>

        {isOpen && (
          <div
            className={cn(
              "absolute z-50 mt-1 max-h-72 w-full overflow-hidden rounded-lg border shadow-lg",
              isLight ? "border-stone-200 bg-white" : "border-white/[0.06] bg-[#363230]",
            )}
          >
            <div className={cn("border-b p-2", isLight ? "border-stone-200" : "border-white/[0.06]")}>
              <input
                autoFocus
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search voices..."
                className={cn(
                  "w-full rounded-md border px-2 py-1.5 text-sm outline-none",
                  isLight
                    ? "border-stone-200 bg-stone-50 text-stone-900 placeholder:text-stone-400 focus:border-blue-500"
                    : "border-white/[0.06] bg-white/[0.04] text-stone-100 placeholder:text-stone-500 focus:border-blue-600",
                )}
              />
            </div>
            <div className="max-h-60 overflow-y-auto p-1">
              {filtered.length === 0 ? (
                <div className={cn("px-2 py-6 text-center text-sm", isLight ? "text-stone-500" : "text-stone-400")}>No voice found.</div>
              ) : (
                filtered.map((voice) => (
                  <VoicePickerItem
                    key={voice.voiceId}
                    voice={voice}
                    isSelected={value === voice.voiceId}
                    onSelect={() => {
                      onValueChange?.(voice.voiceId!)
                      setIsOpen?.(false)
                    }}
                  />
                ))
              )}
            </div>
          </div>
        )}
      </div>
    </AudioPlayerProvider>
  )
}

interface VoicePickerItemProps {
  voice: ElevenLabs.Voice
  isSelected: boolean
  onSelect: () => void
}

function VoicePickerItem({
  voice,
  isSelected,
  onSelect,
}: VoicePickerItemProps) {
  const [isHovered, setIsHovered] = React.useState(false)
  const isLight = useIsLight()
  const orbColors: [string, string] = ["#000e2e", "#004cff"]
  const player = useAudioPlayer()

  const preview = voice.previewUrl
  const audioItem = React.useMemo(
    () => (preview ? { id: voice.voiceId!, src: preview, data: voice } : null),
    [preview, voice],
  )

  const isPlaying =
    audioItem && player.isItemActive(audioItem.id) && player.isPlaying

  const handlePreview = React.useCallback(
    async (e: React.MouseEvent) => {
      e.preventDefault()
      e.stopPropagation()

      if (!audioItem) return

      if (isPlaying) {
        player.pause()
      } else {
        player.play(audioItem)
      }
    },
    [audioItem, isPlaying, player],
  )

  return (
    <button
      type="button"
      onClick={onSelect}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors",
        isLight ? "hover:bg-stone-100" : "hover:bg-white/[0.06]",
        isSelected && (isLight ? "bg-stone-100" : "bg-white/[0.08]"),
      )}
    >
      <div
        className="relative z-10 size-8 shrink-0 cursor-pointer overflow-visible"
        onClick={handlePreview}
      >
        <Orb
          colors={orbColors}
          agentState={isPlaying ? "talking" : undefined}
          className="pointer-events-none absolute inset-0"
        />
        {preview && isHovered && (
          <div className="pointer-events-none absolute inset-0 flex size-8 shrink-0 items-center justify-center rounded-full bg-black/40 backdrop-blur-sm transition-opacity hover:bg-black/50">
            {isPlaying ? (
              <Pause className="size-3 text-white" />
            ) : (
              <Play className="size-3 text-white" />
            )}
          </div>
        )}
      </div>

      <div className="flex flex-1 flex-col gap-0.5 overflow-hidden">
        <span className={cn("truncate font-medium text-sm", isLight ? "text-stone-900" : "text-stone-100")}>{voice.name}</span>
        {voice.labels && (
          <div className={cn("flex items-center gap-1.5 text-xs", isLight ? "text-stone-500" : "text-stone-400")}>
            {voice.labels.accent && <span>{voice.labels.accent}</span>}
            {voice.labels.gender && <span>•</span>}
            {voice.labels.gender && (
              <span className="capitalize">{voice.labels.gender}</span>
            )}
            {voice.labels.age && <span>•</span>}
            {voice.labels.age && (
              <span className="capitalize">{voice.labels.age}</span>
            )}
          </div>
        )}
      </div>

      <Check
        className={cn(
          "ml-auto size-4 shrink-0",
          isSelected ? "opacity-100" : "opacity-0",
          isLight ? "text-stone-900" : "text-stone-100",
        )}
      />
    </button>
  )
}

export { VoicePicker, VoicePickerItem }
