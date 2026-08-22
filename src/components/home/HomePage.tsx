import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readFile } from "@tauri-apps/plugin-fs";
import {
  ArrowCounterClockwise,
  CalendarBlank,
  Check,
  CircleNotch,
  Copy,
  Lectern,
  Pause,
  Play,
  Speedometer,
  Trash,
  UserSound,
} from "@phosphor-icons/react";
import NumberFlow from "@number-flow/react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  commands,
  events,
  type HistoryEntry,
  type HistoryUpdatePayload,
} from "@/bindings";
import { useOsType } from "@/hooks/useOsType";
import type { OSType } from "@/lib/utils/keyboard";
import {
  computeJournalStats,
  countWords,
  formatClock,
  formatDuration,
  formatTimeOfDay,
  groupEntriesByRecency,
} from "@/lib/utils/journalStats";

const PAGE_SIZE = 30;
const DURATION_CONCURRENCY = 6;

/** Only one transcript plays at a time across the whole page. */
let activeAudio: HTMLAudioElement | null = null;

/** Resolve a recording's playable URL (asset protocol; blob fallback on Linux). */
const getAudioUrl = async (
  fileName: string,
  osType: OSType,
): Promise<string | null> => {
  try {
    const result = await commands.getAudioFilePath(fileName);
    if (result.status !== "ok") return null;
    if (osType === "linux") {
      const fileData = await readFile(result.data);
      return URL.createObjectURL(new Blob([fileData], { type: "audio/wav" }));
    }
    return convertFileSrc(result.data, "asset");
  } catch (error) {
    console.error("Failed to resolve audio file:", error);
    return null;
  }
};

/** Real audio duration from the recorded file's metadata. */
const measureAudioDuration = (url: string): Promise<number | null> =>
  new Promise((resolve) => {
    const audio = document.createElement("audio");
    audio.preload = "metadata";
    let settled = false;
    const finish = (value: number | null) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      audio.removeAttribute("src");
      resolve(value);
    };
    const timer = setTimeout(() => finish(null), 15000);
    audio.onloadedmetadata = () => {
      const duration = audio.duration;
      finish(Number.isFinite(duration) && duration > 0 ? duration : null);
    };
    audio.onerror = () => finish(null);
    audio.src = url;
  });

/* ------------------------------------------------------------------ */
/* Transcript row                                                      */
/* ------------------------------------------------------------------ */

const ActionIconButton: React.FC<{
  onClick: () => void;
  title: string;
  disabled?: boolean;
  /** Destructive action — rose hover instead of the neutral/blue one. */
  danger?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, disabled, danger, children }) => (
  <button
    type="button"
    onClick={onClick}
    disabled={disabled}
    title={title}
    aria-label={title}
    className={`flex items-center justify-center rounded-md p-1.5 transition-colors duration-150 disabled:cursor-not-allowed disabled:text-stone-600 ${
      disabled
        ? ""
        : danger
          ? "text-stone-500 hover:bg-rose-600/10 hover:text-rose-600"
          : "text-stone-500 hover:bg-stone-700/60 hover:text-blue-500"
    }`}
  >
    {children}
  </button>
);

interface TranscriptRowProps {
  entry: HistoryEntry;
  duration?: number;
  restoring?: boolean;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  onDelete: (id: number) => Promise<void>;
  onRetry: (id: number) => Promise<void>;
}

const TranscriptRow: React.FC<TranscriptRowProps> = ({
  entry,
  duration,
  restoring = false,
  getAudioUrl,
  onDelete,
  onRetry,
}) => {
  const { t, i18n } = useTranslation();
  const [copied, setCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [playing, setPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  // A failed transcription is one with no text — the audio is always recorded.
  const hasText = entry.transcription_text.trim().length > 0;
  // True while a transcription (manual retry or background restore) is running.
  const busy = retrying || restoring;

  useEffect(
    () => () => {
      if (audioRef.current) {
        audioRef.current.pause();
        audioRef.current = null;
      }
    },
    [],
  );

  const handleCopy = () => {
    if (!hasText) return;
    navigator.clipboard.writeText(entry.transcription_text).catch((error) => {
      console.error("Failed to copy to clipboard:", error);
    });
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const stopPlayback = useCallback(() => {
    audioRef.current?.pause();
    audioRef.current = null;
    setPlaying(false);
  }, []);

  const handleTogglePlayback = async () => {
    if (playing) {
      stopPlayback();
      return;
    }
    const url = await getAudioUrl(entry.file_name);
    if (!url) return;
    activeAudio?.pause();
    const audio = new Audio(url);
    audioRef.current = audio;
    activeAudio = audio;
    audio.onended = () => stopPlayback();
    audio.onerror = () => stopPlayback();
    setPlaying(true);
    audio.play().catch(() => stopPlayback());
  };

  const handleDelete = async () => {
    try {
      await onDelete(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("settings.history.deleteError"));
    }
  };

  const handleRetry = async () => {
    if (retrying) return;
    try {
      setRetrying(true);
      await onRetry(entry.id);
    } catch (error) {
      console.error("Failed to re-transcribe:", error);
      toast.error(t("settings.history.retranscribeError"));
    } finally {
      setRetrying(false);
    }
  };

  const words = countWords(entry.transcription_text);

  return (
    <article className="px-4 py-4">
      <style>{`
        @keyframes home-transcribe-pulse {
          0%, 100% { color: color-mix(in srgb, var(--color-text) 40%, transparent); }
          50% { color: color-mix(in srgb, var(--color-text) 90%, transparent); }
        }
      `}</style>

      {/* Single header row: time + status chips on the left; word badge,
          duration and every action aligned horizontally on the right. */}
      <div className="flex items-center justify-between gap-3">
        <span className="flex min-w-0 items-center gap-2">
          <span className="shrink-0 text-sm font-medium tracking-tight text-stone-100">
            {formatTimeOfDay(entry.timestamp, i18n.language)}
          </span>
          {busy && (
            <span className="inline-flex items-center gap-1 rounded-md bg-blue-500/10 px-1.5 py-0.5 text-[11px] tracking-wide text-blue-300">
              <CircleNotch size={12} className="animate-spin" />
              {t("settings.history.transcribing")}
            </span>
          )}
          {/* Failed transcriptions stay compact: an inline chip next to the
              time instead of a full placeholder line padding the row out. */}
          {!hasText && !busy && (
            <span className="inline-flex items-center rounded-md bg-rose-500/10 px-1.5 py-0.5 text-[11px] tracking-wide text-rose-300">
              {t("settings.history.transcriptionFailed")}
            </span>
          )}
        </span>
        <span className="flex shrink-0 items-center gap-1">
          {!busy && hasText && (
            <>
              <span className="inline-flex items-center rounded-[3.5px] bg-blue-500/[0.11] px-1.5 py-0.5 text-[11px] font-medium leading-none tracking-tight text-blue-300">
                {t("home.words", { count: words })}
              </span>
              {duration !== undefined && (
                <span className="px-1 text-xs tabular-nums tracking-wide text-mid-gray">
                  {formatClock(duration)}
                </span>
              )}
            </>
          )}
          <div className="-mr-1 flex items-center gap-0.5">
            <ActionIconButton
              onClick={handleCopy}
              disabled={!hasText || busy}
              title={t("settings.history.copyToClipboard")}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </ActionIconButton>
            <ActionIconButton
              onClick={handleTogglePlayback}
              title={playing ? t("home.pause") : t("home.play")}
            >
              {playing ? <Pause size={14} /> : <Play size={14} />}
            </ActionIconButton>
            {!hasText && !busy && (
              <ActionIconButton
                onClick={handleRetry}
                title={t("settings.history.retranscribe")}
              >
                <ArrowCounterClockwise size={14} />
              </ActionIconButton>
            )}
            <ActionIconButton
              onClick={handleDelete}
              disabled={busy}
              danger
              title={t("settings.history.delete")}
            >
              <Trash size={14} />
            </ActionIconButton>
          </div>
        </span>
      </div>

      {busy ? (
        <p
          className="mt-2 text-sm text-stone-500"
          style={{ animation: "home-transcribe-pulse 3s ease-in-out infinite" }}
        >
          {t("settings.history.transcribing")}
        </p>
      ) : hasText ? (
        <p className="mt-2 select-text whitespace-pre-wrap break-words text-sm leading-6 text-stone-200">
          {entry.transcription_text}
        </p>
      ) : null}
    </article>
  );
};

/* ------------------------------------------------------------------ */
/* Page                                                                */
/* ------------------------------------------------------------------ */

export const HomePage: React.FC = () => {
  const { t } = useTranslation();
  const osType = useOsType();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [durations, setDurations] = useState<Record<number, number>>({});
  const [restoringIds, setRestoringIds] = useState<Set<number>>(new Set());
  const attemptedDurationsRef = useRef<Set<number>>(new Set());

  const loadAllEntries = useCallback(async () => {
    setLoading(true);
    try {
      const all: HistoryEntry[] = [];
      let cursor: number | undefined;
      let hasMore = true;
      while (hasMore) {
        const result = await commands.getHistoryEntries(
          cursor ?? null,
          PAGE_SIZE,
        );
        if (result.status !== "ok") break;
        all.push(...result.data.entries);
        hasMore = result.data.has_more;
        const last = result.data.entries[result.data.entries.length - 1];
        if (!last) break;
        cursor = last.id;
      }
      setEntries(all);
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAllEntries();
  }, [loadAllEntries]);

  // Live updates from the transcription pipeline.
  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen((event) => {
      const payload: HistoryUpdatePayload = event.payload;
      if (payload.action === "added") {
        setEntries((prev) => [payload.entry, ...prev]);
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      } else if (payload.action === "deleted") {
        setEntries((prev) => prev.filter((e) => e.id !== payload.id));
      }
      // "toggled" is not shown on this page.
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Background restore progress (auto re-transcription of failed entries).
  // The chain reports `started` (fresh-load retries), then `fallback` (a
  // different downloaded model took over) before `completed`/`failed` — the
  // card stays in its transcribing state until one of those two arrives.
  useEffect(() => {
    const unlisten = listen<{ id: number; status: string }>(
      "history-retranscribe",
      (event) => {
        const { id, status } = event.payload;
        setRestoringIds((prev) => {
          const next = new Set(prev);
          if (status === "started" || status === "fallback") {
            next.add(id);
          } else {
            next.delete(id);
          }
          return next;
        });
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Measure real audio durations progressively (bounded concurrency).
  useEffect(() => {
    if (loading) return;
    const pending = entries.filter(
      (entry) => !attemptedDurationsRef.current.has(entry.id),
    );
    if (pending.length === 0) return;

    let cancelled = false;
    let index = 0;
    const worker = async () => {
      while (!cancelled) {
        const current = pending[index++];
        if (!current) return;
        attemptedDurationsRef.current.add(current.id);
        const url = await getAudioUrl(current.file_name, osType);
        if (!url) continue;
        const seconds = await measureAudioDuration(url);
        if (url.startsWith("blob:")) URL.revokeObjectURL(url);
        if (!cancelled && seconds !== null) {
          setDurations((prev) => ({ ...prev, [current.id]: seconds }));
        }
      }
    };
    void Promise.all(
      Array.from({ length: DURATION_CONCURRENCY }, () => worker()),
    );
    return () => {
      cancelled = true;
    };
  }, [entries, loading, osType]);

  const stats = useMemo(
    () => computeJournalStats(entries, durations),
    [entries, durations],
  );

  const groups = useMemo(() => groupEntriesByRecency(entries), [entries]);

  const resolveAudioUrl = useCallback(
    (fileName: string) => getAudioUrl(fileName, osType),
    [osType],
  );

  const deleteEntry = useCallback(async (id: number) => {
    setEntries((prev) => prev.filter((entry) => entry.id !== id));
    const result = await commands.deleteHistoryEntry(id);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }
  }, []);

  const retryTranscription = useCallback(async (id: number) => {
    const result = await commands.retryHistoryEntryTranscription(id);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }
  }, []);

  const statCells = [
    {
      key: "words",
      icon: UserSound,
      value: (
        <NumberFlow value={stats.totalWords} format={{ useGrouping: true }} />
      ),
      label: t("home.stats.words"),
    },
    {
      key: "wpm",
      icon: Speedometer,
      value: stats.avgWpm !== null ? <NumberFlow value={stats.avgWpm} /> : "—",
      label: t("home.stats.wpm"),
    },
    {
      key: "streak",
      icon: CalendarBlank,
      value: <NumberFlow value={stats.dayStreak} />,
      label: t("home.stats.streak"),
    },
    {
      key: "saved",
      icon: Lectern,
      value:
        stats.savedSeconds !== null ? formatDuration(stats.savedSeconds) : "—",
      label: t("home.stats.saved"),
    },
  ];

  const dateGroups = [
    { key: "today", label: t("home.today"), rows: groups.today },
    { key: "yesterday", label: t("home.yesterday"), rows: groups.yesterday },
    { key: "earlier", label: t("home.earlier"), rows: groups.earlier },
  ].filter((group) => group.rows.length > 0);

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6">
      <div className="space-y-2">
        <h2 className="px-4 text-xs font-medium uppercase tracking-wide text-mid-gray">
          {t("home.title")}
        </h2>

        {/* Stats — quiet surface, vertical hairlines between cells, and a
            single bottom hairline separating the saved-time footer. No outer
            border, no shadows, no colored icon chips. */}
        <section className="rounded-[10px] bg-surface">
          <div className="grid grid-cols-2 sm:grid-cols-4 sm:divide-x sm:divide-divider">
            {statCells.map((cell) => (
              <div
                key={cell.key}
                className="flex min-w-0 flex-col items-center gap-2 px-4 py-5"
              >
                <cell.icon size={24} weight="light" className="text-stone-400" />
                <span className="truncate text-xl font-medium tracking-tight text-stone-50">
                  {cell.value}
                </span>
                <span className="text-xs tracking-wide text-mid-gray">
                  {cell.label}
                </span>
              </div>
            ))}
          </div>
          <div className="border-t border-divider px-4 py-3 text-center text-sm text-stone-400">
            {stats.savedSeconds !== null && stats.savedSeconds > 0 ? (
              <>
                {t("home.savedLinePrefix")}{" "}
                <span className="font-medium text-stone-200">
                  {formatDuration(stats.savedSeconds)}
                </span>{" "}
                {t("home.savedLineSuffix")}
              </>
            ) : (
              t("home.savedLineEmpty")
            )}
          </div>
        </section>

        {/* Transcripts — grouped Today / Yesterday / Earlier */}
        {loading ? (
          <div className="rounded-[10px] bg-surface px-4 py-8 text-center text-sm text-mid-gray">
            {t("settings.history.loading")}
          </div>
        ) : entries.length === 0 ? (
          <div className="rounded-[10px] bg-surface px-4 py-8 text-center text-sm text-mid-gray">
            {t("home.empty")}
          </div>
        ) : (
          dateGroups.map((group) => (
            <section
              key={group.key}
              className="overflow-hidden rounded-[10px] bg-surface"
            >
              <header className="flex items-center justify-between border-b border-divider px-4 py-2.5">
                <h3 className="text-xs font-medium uppercase tracking-wide text-mid-gray">
                  {group.label}
                </h3>
                <span className="text-xs text-mid-gray/70">
                  {group.rows.length}
                </span>
              </header>
              <div className="divide-y divide-divider/60">
                {group.rows.map((entry) => (
                  <TranscriptRow
                    key={entry.id}
                    entry={entry}
                    duration={durations[entry.id]}
                    restoring={restoringIds.has(entry.id)}
                    getAudioUrl={resolveAudioUrl}
                    onDelete={deleteEntry}
                    onRetry={retryTranscription}
                  />
                ))}
              </div>
            </section>
          ))
        )}
      </div>
    </div>
  );
};
