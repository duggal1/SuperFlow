import type { HistoryEntry } from "@/bindings";

/**
 * Average adult typing speed (words per minute) used as the explicit,
 * labeled baseline for all "time saved versus typing" math.
 */
export const TYPING_WPM = 40;

/** Count whitespace-separated words in a transcription. */
export const countWords = (text: string): number => {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
};

/** User-visible output after every enabled transcript-processing stage. */
export const getFinalTranscriptionText = (entry: HistoryEntry): string => {
  const processed = entry.post_processed_text;
  return processed?.trim() ? processed : entry.transcription_text;
};

/** Local calendar date key (YYYY-MM-DD) for a unix timestamp in seconds. */
export const localDateKey = (timestampSec: number): string => {
  const date = new Date(timestampSec * 1000);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

/**
 * Consecutive-day streak ending at today (or yesterday, when today has no
 * entries yet). A gap of one full day breaks the streak.
 */
export const computeDayStreak = (
  timestampsSec: number[],
  now: Date = new Date(),
): number => {
  const days = new Set(timestampsSec.map((ts) => localDateKey(ts)));
  const cursor = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  const key = (date: Date) => localDateKey(date.getTime() / 1000);

  // A streak is still alive if it started yesterday.
  if (!days.has(key(cursor))) {
    cursor.setDate(cursor.getDate() - 1);
  }

  let streak = 0;
  while (days.has(key(cursor))) {
    streak += 1;
    cursor.setDate(cursor.getDate() - 1);
  }
  return streak;
};

export interface EntryGroups<T> {
  today: T[];
  yesterday: T[];
  earlier: T[];
}

/** Split entries (newest first) into Today / Yesterday / Earlier buckets. */
export const groupEntriesByRecency = <T extends { timestamp: number }>(
  entries: T[],
  now: Date = new Date(),
): EntryGroups<T> => {
  const todayKey = localDateKey(Math.floor(now.getTime() / 1000));
  const yesterday = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() - 1,
  );
  const yesterdayKey = localDateKey(yesterday.getTime() / 1000);

  const groups: EntryGroups<T> = { today: [], yesterday: [], earlier: [] };
  for (const entry of entries) {
    const entryKey = localDateKey(entry.timestamp);
    if (entryKey === todayKey) {
      groups.today.push(entry);
    } else if (entryKey === yesterdayKey) {
      groups.yesterday.push(entry);
    } else {
      groups.earlier.push(entry);
    }
  }
  return groups;
};

/** Human duration: "42s", "12m 30s", "3h 24m". */
export const formatDuration = (totalSeconds: number): string => {
  const seconds = Math.max(0, Math.round(totalSeconds));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    const rest = seconds % 60;
    return rest ? `${minutes}m ${rest}s` : `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours}h ${rest}m` : `${hours}h`;
};

/** Clock duration: "3:05" or "1:02:03". */
export const formatClock = (totalSeconds: number): string => {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${m}:${String(s).padStart(2, "0")}`;
};

/** Time of day for a unix timestamp in seconds, localized ("10:42 AM"). */
export const formatTimeOfDay = (timestampSec: number, locale: string): string =>
  new Intl.DateTimeFormat(locale, {
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(timestampSec * 1000));

/**
 * Seconds saved versus typing the same words at TYPING_WPM:
 * typing time minus actual speaking time (from real audio durations).
 */
export const timeSavedSeconds = (
  totalWords: number,
  spokenSeconds: number,
): number =>
  Math.max(0, Math.round((totalWords / TYPING_WPM) * 60 - spokenSeconds));

export interface JournalStats {
  totalWords: number;
  /** Average speaking speed in WPM; null until real audio durations exist. */
  avgWpm: number | null;
  dayStreak: number;
  /** Seconds saved versus typing; null while no audio durations are known. */
  savedSeconds: number | null;
}

/** Compute all journal stats from entries plus measured audio durations.
 *
 * Prefers the per-entry stats persisted by the backend (word_count,
 * audio_duration_secs, avg_wpm, time_saved_secs) so numbers stay correct even
 * when recording files are gone; falls back to live word parsing and the
 * caller-probed `durations` for legacy rows that predate persistence.
 */
export const computeJournalStats = (
  entries: HistoryEntry[],
  durations: Record<number, number>,
): JournalStats => {
  let totalWords = 0;
  let spokenSeconds = 0;
  // Stored time-saved / WPM accumulators (duration-weighted average).
  let storedSavedSeconds = 0;
  let hasStoredSaved = false;
  let wpmDurationWeighted = 0;
  let storedWpmDurationSum = 0;

  for (const entry of entries) {
    const words =
      entry.word_count > 0
        ? entry.word_count
        : countWords(getFinalTranscriptionText(entry));
    totalWords += words;

    const duration = entry.audio_duration_secs ?? durations[entry.id];
    if (duration !== undefined) {
      spokenSeconds += duration;
      if (entry.time_saved_secs !== null) {
        storedSavedSeconds += entry.time_saved_secs;
        hasStoredSaved = true;
      }
      if (entry.avg_wpm !== null && entry.avg_wpm > 0) {
        wpmDurationWeighted += entry.avg_wpm * duration;
        storedWpmDurationSum += duration;
      }
    }
  }

  const dayStreak = computeDayStreak(entries.map((entry) => entry.timestamp));

  const avgWpm =
    storedWpmDurationSum >= 15
      ? Math.round(wpmDurationWeighted / storedWpmDurationSum)
      : spokenSeconds >= 15
        ? Math.round(totalWords / (spokenSeconds / 60))
        : null;

  const savedSeconds = hasStoredSaved
    ? Math.max(0, Math.round(storedSavedSeconds))
    : spokenSeconds > 0
      ? timeSavedSeconds(totalWords, spokenSeconds)
      : null;

  return { totalWords, avgWpm, dayStreak, savedSeconds };
};
