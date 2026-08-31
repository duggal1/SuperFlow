import React, { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readFile } from "@tauri-apps/plugin-fs";
import {
  Check,
  CircleNotch,
  Copy,
  FolderOpen,
  ArrowCounterClockwise,
  Trash,
} from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  commands,
  events,
  type HistoryEntry,
  type HistoryUpdatePayload,
} from "@/bindings";
import { useOsType } from "@/hooks/useOsType";
import { formatDateTime } from "@/utils/dateFormat";
import { AudioPlayer, AudioPlayerGroup } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";
import { Badge } from "../../ui/Badge";
import { ExportFormatSelector } from "../ExportFormatSelector";
import { useIsLight } from "@/lib/utils/theme";

const IconButton: React.FC<{
  onClick: () => void;
  title: string;
  disabled?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, disabled, children }) => {
  const isLight = useIsLight();
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`flex cursor-pointer items-center justify-center rounded-md p-1.5 transition-colors disabled:cursor-not-allowed ${isLight ? "text-stone-500 hover:bg-stone-100 hover:text-blue-600 disabled:text-stone-300" : "text-text/50 hover:bg-stone-700/60 hover:text-blue-500 disabled:text-text/20"}`}
      title={title}
    >
      {children}
    </button>
  );
};

const PAGE_SIZE = 30;

interface OpenRecordingsButtonProps {
  onClick: () => void;
  label: string;
}

const OpenRecordingsButton: React.FC<OpenRecordingsButtonProps> = ({
  onClick,
  label,
}) => (
  <Button
    onClick={onClick}
    size="sm"
    icon={<FolderOpen className="size-3.5" />}
    title={label}
  >
    {label}
  </Button>
);

export const HistorySettings: React.FC = () => {
  const { t } = useTranslation();
  const osType = useOsType();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [restoringIds, setRestoringIds] = useState<Set<number>>(new Set());
  const [hasMore, setHasMore] = useState(true);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);

  // Keep ref in sync for use in IntersectionObserver callback
  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  const loadPage = useCallback(async (cursor?: number) => {
    const isFirstPage = cursor === undefined;
    if (!isFirstPage && loadingRef.current) return;
    loadingRef.current = true;

    if (isFirstPage) setLoading(true);

    try {
      const result = await commands.getHistoryEntries(
        cursor ?? null,
        PAGE_SIZE,
      );
      if (result.status === "ok") {
        const { entries: newEntries, has_more } = result.data;
        setEntries((prev) =>
          isFirstPage ? newEntries : [...prev, ...newEntries],
        );
        setHasMore(has_more);
      }
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadPage();
  }, [loadPage]);

  // Infinite scroll via IntersectionObserver
  useEffect(() => {
    if (loading) return;

    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;

    const observer = new IntersectionObserver(
      (observerEntries) => {
        const first = observerEntries[0];
        if (first.isIntersecting) {
          const lastEntry = entriesRef.current[entriesRef.current.length - 1];
          if (lastEntry) {
            loadPage(lastEntry.id);
          }
        }
      },
      { threshold: 0 },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loading, hasMore, loadPage]);

  // Listen for new entries added from the transcription pipeline
  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen((event) => {
      const payload: HistoryUpdatePayload = event.payload;
      if (payload.action === "added") {
        setEntries((prev) => [payload.entry, ...prev]);
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      }
      // "deleted" and "toggled" are handled by optimistic updates only,
      // so we intentionally ignore them here to avoid double-mutation.
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Background restore progress (auto re-transcription of failed entries).
  // The chain reports `started` (fresh-load retries), then `fallback` (a
  // different downloaded model took over) before `completed`/`failed` — the
  // row stays busy until one of those two arrives.
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

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const getAudioUrl = useCallback(
    async (fileName: string) => {
      try {
        const result = await commands.getAudioFilePath(fileName);
        if (result.status === "ok") {
          if (osType === "linux") {
            const fileData = await readFile(result.data);
            const blob = new Blob([fileData], { type: "audio/wav" });
            return URL.createObjectURL(blob);
          }
          return convertFileSrc(result.data, "asset");
        }
        return null;
      } catch (error) {
        console.error("Failed to get audio file path:", error);
        return null;
      }
    },
    [osType],
  );

  const deleteAudioEntry = async (id: number) => {
    // Optimistically remove
    setEntries((prev) => prev.filter((e) => e.id !== id));
    try {
      const result = await commands.deleteHistoryEntry(id);
      if (result.status !== "ok") {
        // Reload on failure
        loadPage();
      }
    } catch (error) {
      console.error("Failed to delete entry:", error);
      loadPage();
    }
  };

  const retryHistoryEntry = async (id: number) => {
    const result = await commands.retryHistoryEntryTranscription(id);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }
  };

  const openRecordingsFolder = async () => {
    try {
      const result = await commands.openRecordingsFolder();
      if (result.status !== "ok") {
        throw new Error(String(result.error));
      }
    } catch (error) {
      console.error("Failed to open recordings folder:", error);
    }
  };

  const isLight = useIsLight();
  let content: React.ReactNode;

  if (loading) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.history.loading")}
      </div>
    );
  } else if (entries.length === 0) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.history.empty")}
      </div>
    );
  } else {
    content = (
      <>
        <AudioPlayerGroup>
          <div className={isLight ? "divide-y divide-stone-200/60" : "divide-y divide-stone-700"}>
            {entries.map((entry) => (
              <HistoryEntryComponent
                key={entry.id}
                entry={entry}
                restoring={restoringIds.has(entry.id)}
                onCopyText={() => copyToClipboard(entry.transcription_text)}
                getAudioUrl={getAudioUrl}
                deleteAudio={deleteAudioEntry}
                retryTranscription={retryHistoryEntry}
              />
            ))}
          </div>
        </AudioPlayerGroup>
        {/* Sentinel for infinite scroll */}
        <div ref={sentinelRef} className="h-1" />
      </>
    );
  }
  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <ExportFormatSelector />
      <div className="space-y-2">
        <div className="px-4 flex items-center justify-between">
          <div>
            <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("settings.history.title")}
            </h2>
          </div>
          <OpenRecordingsButton
            onClick={openRecordingsFolder}
            label={t("settings.history.openFolder")}
          />
        </div>
        <div className={`rounded-[10px] bg-surface overflow-visible ${isLight ? "border border-stone-200/80" : ""}`}>
          {content}
        </div>
      </div>
    </div>
  );
};

interface HistoryEntryProps {
  entry: HistoryEntry;
  restoring?: boolean;
  onCopyText: () => void;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
  retryTranscription: (id: number) => Promise<void>;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  restoring = false,
  onCopyText,
  getAudioUrl,
  deleteAudio,
  retryTranscription,
}) => {
  const { t, i18n } = useTranslation();
  const [showCopied, setShowCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);

  // True while a transcription (manual retry or background restore) is running.
  const busy = retrying || restoring;

  const hasTranscription = entry.transcription_text.trim().length > 0;

  const handleLoadAudio = useCallback(
    () => getAudioUrl(entry.file_name),
    [getAudioUrl, entry.file_name],
  );

  const handleCopyText = () => {
    if (!hasTranscription) {
      return;
    }

    onCopyText();
    setShowCopied(true);
    setTimeout(() => setShowCopied(false), 2000);
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("settings.history.deleteError"));
    }
  };

  const handleRetranscribe = async () => {
    try {
      setRetrying(true);
      await retryTranscription(entry.id);
    } catch (error) {
      console.error("Failed to re-transcribe:", error);
      toast.error(t("settings.history.retranscribeError"));
    } finally {
      setRetrying(false);
    }
  };

  const formattedDate = formatDateTime(String(entry.timestamp), i18n.language);
  const isLightRow = useIsLight();

  return (
    <article className="flex flex-col gap-3 px-4 py-4 [content-visibility:auto] [contain-intrinsic-size:auto_156px]">
      <div className="flex items-center justify-between gap-3">
        <span className="flex min-w-0 flex-col items-start gap-1.5">
          <span className="flex items-center gap-2">
            <span className={`shrink-0 text-sm font-medium tracking-tight ${isLightRow ? "text-stone-900" : "text-stone-100"}`}>
              {formattedDate}
            </span>
            {busy && (
              <Badge variant="blue" className="px-1.5 py-0.5 text-[11px]">
                <CircleNotch size={12} className="animate-spin" />
                {t("settings.history.transcribing")}
              </Badge>
            )}
          </span>
          {!hasTranscription && !busy && (
            <Badge variant="rose" className="px-1.5 py-0.5 text-[11px]">
              {t("settings.history.transcriptionFailed")}
            </Badge>
          )}
        </span>
        <div className="flex shrink-0 items-center gap-0.5">
          <IconButton
            onClick={handleCopyText}
            disabled={!hasTranscription || busy}
            title={t("settings.history.copyToClipboard")}
          >
            {showCopied ? (
              <Check width={16} height={16} />
            ) : (
              <Copy width={16} height={16} />
            )}
          </IconButton>
          {!hasTranscription && !busy && (
            <IconButton
              onClick={handleRetranscribe}
              title={t("settings.history.retranscribe")}
            >
              <ArrowCounterClockwise width={16} height={16} />
            </IconButton>
          )}
          <IconButton
            onClick={handleDeleteEntry}
            disabled={busy}
            title={t("settings.history.delete")}
          >
            <Trash width={16} height={16} />
          </IconButton>
        </div>
      </div>

      {(busy || hasTranscription) && (
        <p
          className={`pb-2 text-sm leading-6 ${
            busy
              ? isLightRow ? "text-stone-600" : "text-stone-500"
              : isLightRow ? "select-text whitespace-pre-wrap break-words text-stone-800" : "select-text whitespace-pre-wrap break-words text-stone-200"
          }`}
          style={
            busy
              ? { animation: "home-transcribe-pulse 3s ease-in-out infinite" }
              : undefined
          }
        >
          {busy ? t("settings.history.transcribing") : entry.transcription_text}
        </p>
      )}

      <AudioPlayer onLoadRequest={handleLoadAudio} className="w-full" />
    </article>
  );
};
