// Standalone assert check (no JS unit-test runner in this repo). Run with:
//   bun src/lib/utils/journalStats.test.ts
import assert from "node:assert";
import {
  computeDayStreak,
  computeJournalStats,
  countWords,
  formatClock,
  formatDuration,
  groupEntriesByRecency,
  localDateKey,
  timeSavedSeconds,
} from "./journalStats";

// --- countWords ---
assert.equal(countWords(""), 0);
assert.equal(countWords("   "), 0);
assert.equal(countWords("hello world"), 2);
assert.equal(countWords("  one   two\tthree\nfour "), 4);

// --- localDateKey ---
assert.equal(localDateKey(0), `${new Date(0).getFullYear()}-01-01`);

// --- computeDayStreak ---
const now = new Date(2026, 7, 22, 15, 0, 0); // Aug 22 2026, local time
const day = (offsetDays: number, hour = 12) =>
  Math.floor(new Date(2026, 7, 22 - offsetDays, hour).getTime() / 1000);

// today only -> 1
assert.equal(computeDayStreak([day(0)], now), 1);
// today + yesterday -> 2
assert.equal(computeDayStreak([day(0), day(1)], now), 2);
// yesterday only (today silent) -> 1
assert.equal(computeDayStreak([day(1)], now), 1);
// gap: yesterday + 3 days ago -> 1
assert.equal(computeDayStreak([day(1), day(3)], now), 1);
// nothing recent -> 0
assert.equal(computeDayStreak([day(3)], now), 0);
// duplicates on the same day don't inflate
assert.equal(computeDayStreak([day(0), day(0), day(1), day(2)], now), 3);
// empty -> 0
assert.equal(computeDayStreak([], now), 0);

// --- groupEntriesByRecency ---
const mkEntry = (id: number, timestamp: number) => ({ id, timestamp });
const entries = [mkEntry(1, day(0)), mkEntry(2, day(1)), mkEntry(3, day(5))];
const groups = groupEntriesByRecency(entries, now);
assert.deepEqual(
  groups.today.map((e) => e.id),
  [1],
);
assert.deepEqual(
  groups.yesterday.map((e) => e.id),
  [2],
);
assert.deepEqual(
  groups.earlier.map((e) => e.id),
  [3],
);

// --- formatDuration ---
assert.equal(formatDuration(0), "0s");
assert.equal(formatDuration(42), "42s");
assert.equal(formatDuration(60), "1m");
assert.equal(formatDuration(750), "12m 30s");
assert.equal(formatDuration(12240), "3h 24m");
assert.equal(formatDuration(10800), "3h");

// --- formatClock ---
assert.equal(formatClock(0), "0:00");
assert.equal(formatClock(185), "3:05");
assert.equal(formatClock(3723), "1:02:03");

// --- timeSavedSeconds ---
// 80 words at 40 WPM = 120s typing; spoken 60s -> saved 60s
assert.equal(timeSavedSeconds(80, 60), 60);
// speaking slower than typing baseline -> clamped to 0
assert.equal(timeSavedSeconds(10, 100), 0);

// --- computeJournalStats ---
const mkHistEntry = (id: number, timestamp: number, text: string) => ({
  id,
  file_name: `f${id}.wav`,
  timestamp,
  saved: false,
  title: "",
  transcription_text: text,
  post_processed_text: null,
  post_process_prompt: null,
  post_process_requested: false,
});
const hist = [
  mkHistEntry(1, day(0), "one two three four"),
  mkHistEntry(2, day(1), "five six"),
];
const noDurations = computeJournalStats(hist, {});
assert.equal(noDurations.totalWords, 6);
assert.equal(noDurations.avgWpm, null);
assert.equal(noDurations.savedSeconds, null);
assert.equal(noDurations.dayStreak, 2);

// 6 words at 40 WPM = 9s typing; spoken 18s -> speaking slower than baseline,
// so savings clamp to 0; 6 words / (18/60) min = 20 WPM
const withDurations = computeJournalStats(hist, { 1: 12, 2: 6 });
assert.equal(withDurations.savedSeconds, 0);
assert.equal(withDurations.avgWpm, 20);

console.log("journalStats: all assertions passed");
