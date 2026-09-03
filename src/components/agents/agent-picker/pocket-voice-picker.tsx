"use client";

import * as React from "react";
import { Check, ChevronsUpDown } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { IOSSpinner } from "@/components/shared/global-spinner";
import { useIsLight } from "@/lib/utils/theme";
import { Orb } from "./orb";
import { cn } from "@/lib/utils";

export interface PocketVoice {
  id: string;
  name: string;
}

interface PocketVoicePickerProps {
  voices: PocketVoice[];
  value?: string;
  onValueChange?: (value: string) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
}

function voiceSeed(id: string): number {
  let h = 0;
  for (let i = 0; i < id.length; i++) {
    h = (h * 31 + id.charCodeAt(i)) | 0;
  }
  return h;
}

/**
 * Custom Pocket-TTS voice selector. Native React dropdown (no base-ui), real
 * Orb per row, voices pulled from the backend (`tts_voices`). DESIGN.md
 * tokens only: Button secondary trigger, Badge-less rows, stone surfaces.
 */
export function PocketVoicePicker({
  voices,
  value,
  onValueChange,
  placeholder = "Select a voice...",
  className,
  disabled = false,
}: PocketVoicePickerProps) {
  const isLight = useIsLight();
  const [open, setOpen] = React.useState(false);
  const [search, setSearch] = React.useState("");
  const containerRef = React.useRef<HTMLDivElement>(null);

  const selected = voices.find((v) => v.id === value);
  const orbColors: [string, string] = React.useMemo(
    () => ["#BCCFF7", "#144FFF"],
    [],
  );

  const filtered = React.useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return voices;
    return voices.filter((v) => v.name.toLowerCase().includes(q));
  }, [voices, search]);

  React.useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open ]);

  React.useEffect(() => {
    if (!open) setSearch("");
  }, [open ]);

  return (
    <div ref={containerRef} className={cn("relative w-full", className)}>
      <Button
        variant="secondary"
        size="sm"
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        className="!h-9 w-full !justify-between !px-3"
        icon={
          selected ? (
            <span className="relative size-6 shrink-0 overflow-visible">
              <Orb colors={orbColors} agentState="thinking" className="absolute inset-0" />
            </span>
          ) : undefined
        }
      >
        <span className="flex min-w-0 flex-1 items-center justify-between gap-2">
          <span className="truncate">{selected ? selected.name : placeholder}</span>
          <ChevronsUpDown className="size-4 shrink-0 opacity-50" />
        </span>
      </Button>

      {open && (
        <div
          className={cn(
            "absolute z-50 mt-1 max-h-72 w-full overflow-hidden rounded-lg border shadow-lg",
            isLight ? "border-stone-200/70 bg-white" : "border-white/[0.06] bg-[#363230]",
          )}
        >
          <div className={cn("border-b p-2", isLight ? "border-stone-200/70" : "border-white/[0.06]")}>
            <input
              autoFocus
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search voices..."
              className={cn(
                "w-full rounded-md border px-2 py-1.5 text-sm outline-none",
                isLight
                  ? "border-stone-200/70 bg-stone-50 text-stone-900 placeholder:text-stone-400 focus:border-blue-500"
                  : "border-white/[0.06] bg-white/[0.04] text-stone-100 placeholder:text-stone-500 focus:border-blue-600",
              )}
            />
          </div>
          <div className="max-h-60 overflow-y-auto p-1">
            {voices.length === 0 ? (
              <div className="flex items-center justify-center gap-2 px-2 py-6">
                <IOSSpinner size={14} />
              </div>
            ) : filtered.length === 0 ? (
              <div className={cn("px-2 py-6 text-center text-sm", isLight ? "text-stone-500" : "text-stone-400")}>
                No voice found.
              </div>
            ) : (
              filtered.map((voice) => {
                const isSelected = value === voice.id;
                return (
                  <button
                    key={voice.id}
                    type="button"
                    onClick={() => {
                      onValueChange?.(voice.id);
                      setOpen(false);
                    }}
                    className={cn(
                      "flex w-full cursor-pointer items-center gap-3 rounded-md px-2 py-2 text-left transition-colors",
                      isLight ? "hover:bg-stone-100/80" : "hover:bg-white/[0.08]",
                      isSelected && (isLight ? "bg-stone-100/80" : "bg-white/[0.08]"),
                    )}
                  >
                    <span className="relative size-8 shrink-0 overflow-visible">
                      <Orb
                        colors={orbColors}
                        seed={voiceSeed(voice.id)}
                        agentState={isSelected ? "thinking" : null}
                        className="pointer-events-none absolute inset-0"
                      />
                    </span>
                    <span className={cn("flex-1 truncate text-sm font-medium", isLight ? "text-stone-900" : "text-stone-100")}>
                      {voice.name}
                    </span>
                    <Check className={cn("ml-auto size-4 shrink-0", isSelected ? "opacity-100" : "opacity-0", isLight ? "text-stone-900" : "text-stone-100")} />
                  </button>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}
