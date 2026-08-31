import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  ArrowUp02Icon,
  Download01Icon,
  File02Icon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import ReactMarkdown from "react-markdown";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { Badge, type BadgeVariant } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { useIsLight } from "@/lib/utils/theme";

interface MeetingSegment {
  speaker: string;
  start_ms: number;
  end_ms: number;
  text: string;
}

interface IntelligenceItem {
  issue: string;
  timestamp: string | null;
  evidence: string | null;
  why_it_matters: string | null;
  better_approach: string | null;
}

interface MeetingIntelligence {
  meeting_type: string;
  outcome: string;
  what_went_well: IntelligenceItem[];
  mistakes: IntelligenceItem[];
  missed_opportunities: IntelligenceItem[];
  communication_issues: IntelligenceItem[];
  important_decisions: string[];
  action_items: string[];
  risks: string[];
  lessons: string[];
  next_time: string[];
}

interface MeetingRecord {
  id: string;
  title: string;
  started_at: number;
  ended_at: number;
  duration_ms: number;
  transcript: MeetingSegment[];
  created_at: number;
  intelligence: MeetingIntelligence | null;
}

interface MeetingListEntry {
  id: string;
  title: string;
  started_at: number;
  duration_ms: number;
  has_intelligence: boolean;
}

type DetailTab = "transcript" | "intelligence" | "ask";

const formatDuration = (milliseconds: number) => {
  const seconds = Math.max(0, Math.round(milliseconds / 1_000));
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const remainder = seconds % 60;
  const parts: string[] = [];
  if (hours > 0) parts.push(`${hours} ${hours === 1 ? "hour" : "hours"}`);
  if (minutes > 0)
    parts.push(`${minutes} ${minutes === 1 ? "minute" : "minutes"}`);
  if (hours === 0 && (remainder > 0 || minutes === 0))
    parts.push(`${remainder} ${remainder === 1 ? "second" : "seconds"}`);
  return parts.join(" ");
};

const formatTimestamp = (milliseconds: number) => {
  const seconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`
    : `${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`;
};

const formatMeetingDate = (timestamp: number) => {
  const date = new Date(timestamp);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  const sameDay = (left: Date, right: Date) =>
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate();
  const time = new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
  if (sameDay(date, today)) return `Today · ${time}`;
  if (sameDay(date, yesterday)) return `Yesterday · ${time}`;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(date);
};

const actionClass =
  "inline-flex h-8 items-center justify-center gap-1.5 rounded-lg px-3 text-[13px] font-medium text-stone-500 transition-colors duration-150 hover:bg-surface-hover hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-blue-500 disabled:cursor-not-allowed disabled:opacity-45";

const IntelligenceFinding = ({
  item,
  label,
  variant,
}: {
  item: IntelligenceItem;
  label: string;
  variant: BadgeVariant;
}) => (
  <article className="py-4 first:pt-0 last:pb-0">
    <div className="mb-2 flex items-center gap-2">
      <Badge variant={variant} className="text-[11px]">
        {label}
      </Badge>
      {item.timestamp && (
        <span className="font-mono text-[11px] text-stone-500">
          {item.timestamp}
        </span>
      )}
    </div>
    <h4 className="text-[14px] font-medium text-text">{item.issue}</h4>
    {item.evidence && (
      <p className="mt-1.5 text-[13px] leading-5 text-stone-500">
        {item.evidence}
      </p>
    )}
    {item.why_it_matters && (
      <p className="mt-1.5 text-[13px] leading-5 text-stone-400">
        {item.why_it_matters}
      </p>
    )}
    {item.better_approach && (
      <p className="mt-2 text-[13px] leading-5 text-text">
        {item.better_approach}
      </p>
    )}
  </article>
);

const TextSection = ({ title, items }: { title: string; items: string[] }) => {
  if (items.length === 0) return null;
  return (
    <section className="py-6">
      <h3 className="text-[15px] font-medium text-text">{title}</h3>
      <ul className="mt-3 space-y-2.5">
        {items.map((item) => (
          <li
            key={item}
            className="flex gap-3 text-[13px] leading-5 text-stone-500"
          >
            <span
              aria-hidden="true"
              className="mt-2 size-1 shrink-0 rounded-[1px] bg-blue-600"
            />
            <span>{item}</span>
          </li>
        ))}
      </ul>
    </section>
  );
};

const timestampToMilliseconds = (timestamp: string) => {
  const parts = timestamp.split(":").map(Number);
  if (parts.some(Number.isNaN)) return null;
  const [hours, minutes, seconds] =
    parts.length === 3 ? parts : [0, parts[0], parts[1]];
  return ((hours * 60 + minutes) * 60 + seconds) * 1_000;
};

const MeetingAnswer = ({
  answer,
  transcript,
  onTimestamp,
}: {
  answer: string;
  transcript: MeetingSegment[];
  onTimestamp: (startMilliseconds: number) => void;
}) => {
  const timestampPattern = /\[(?:(?:\d{1,2}):)?\d{1,2}:\d{2}\]/g;
  const markdown = answer.replace(timestampPattern, (match) => {
    const milliseconds = timestampToMilliseconds(match.slice(1, -1));
    return milliseconds === null
      ? match
      : `[${match}](#meeting-timestamp-${milliseconds})`;
  });
  return (
    <ReactMarkdown
      components={{
        h1: ({ children }) => (
          <h1 className="mb-3 mt-6 text-lg font-medium text-text first:mt-0">
            {children}
          </h1>
        ),
        h2: ({ children }) => (
          <h2 className="mb-2 mt-6 text-[15px] font-medium text-text first:mt-0">
            {children}
          </h2>
        ),
        h3: ({ children }) => (
          <h3 className="mb-2 mt-5 text-[14px] font-medium text-text">
            {children}
          </h3>
        ),
        p: ({ children }) => <p className="my-3 first:mt-0">{children}</p>,
        ul: ({ children }) => (
          <ul className="my-3 space-y-2 [&>li]:relative [&>li]:pl-4 [&>li]:before:absolute [&>li]:before:left-0 [&>li]:before:top-[0.62em] [&>li]:before:size-1 [&>li]:before:rounded-[1px] [&>li]:before:bg-blue-600">
            {children}
          </ul>
        ),
        ol: ({ children }) => (
          <ol className="my-3 list-decimal space-y-2 pl-5 marker:text-blue-600">
            {children}
          </ol>
        ),
        li: ({ children }) => <li className="min-w-0">{children}</li>,
        strong: ({ children }) => (
          <strong className="font-medium text-text">{children}</strong>
        ),
        code: ({ children }) => (
          <code className="rounded-[3px] bg-stone-500/10 px-1 py-0.5 font-mono text-[12px] text-text">
            {children}
          </code>
        ),
        a: ({ href, children }) => {
          const prefix = "#meeting-timestamp-";
          if (!href?.startsWith(prefix)) {
            return (
              <a
                href={href}
                className="text-blue-500 underline underline-offset-2"
              >
                {children}
              </a>
            );
          }
          const milliseconds = Number(href.slice(prefix.length));
          const nearest = transcript.reduce<MeetingSegment | null>(
            (closest, segment) => {
              if (!closest) return segment;
              return Math.abs(segment.start_ms - milliseconds) <
                Math.abs(closest.start_ms - milliseconds)
                ? segment
                : closest;
            },
            null,
          );
          return (
            <button
              type="button"
              onClick={() => nearest && onTimestamp(nearest.start_ms)}
              className="mx-0.5 inline-flex cursor-pointer rounded-[3px] px-1 font-mono text-[12px] text-blue-500 transition-colors duration-150 hover:bg-blue-500/10 hover:text-blue-400 focus-visible:outline-none"
            >
              {children}
            </button>
          );
        },
      }}
    >
      {markdown}
    </ReactMarkdown>
  );
};

export function MeetingPage() {
  const { t } = useTranslation();
  const isLight = useIsLight();
  const tabs: Array<{ id: DetailTab; label: string }> = [
    { id: "transcript", label: t("meeting.tabs.transcript") },
    { id: "intelligence", label: t("meeting.tabs.intelligence") },
    { id: "ask", label: t("meeting.tabs.ask") },
  ];
  const [meetings, setMeetings] = useState<MeetingListEntry[]>([]);
  const [selected, setSelected] = useState<MeetingRecord | null>(null);
  const [tab, setTab] = useState<DetailTab>("transcript");
  const [loading, setLoading] = useState(true);
  const [analyzing, setAnalyzing] = useState(false);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const [search, setSearch] = useState("");
  const [meetingSearch, setMeetingSearch] = useState("");
  const analysisAttemptedRef = useRef<string | null>(null);

  const loadMeetings = useCallback(async () => {
    try {
      setMeetings(
        await invoke<MeetingListEntry[]>("list_meetings", {
          limit: 100,
          offset: 0,
        }),
      );
    } catch (error) {
      toast.error("Could not load meetings", { description: String(error) });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMeetings();
    const unlisten = listen<string>("meeting-saved", () => void loadMeetings());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [loadMeetings]);

  const openMeeting = async (id: string, nextTab: DetailTab = "transcript") => {
    try {
      const meeting = await invoke<MeetingRecord | null>("get_meeting", { id });
      if (!meeting) throw new Error("Meeting not found");
      setSelected(meeting);
      setTab(nextTab);
      setSearch("");
      setAnswer(null);
      analysisAttemptedRef.current = null;
    } catch (error) {
      toast.error("Could not open meeting", { description: String(error) });
    }
  };

  const analyze = useCallback(async () => {
    if (!selected || selected.intelligence || analyzing) return;
    setAnalyzing(true);
    try {
      const meeting = await invoke<MeetingRecord>(
        "generate_meeting_intelligence",
        {
          id: selected.id,
        },
      );
      setSelected(meeting);
      await loadMeetings();
    } catch (error) {
      toast.error("Intelligence generation failed", {
        description: String(error),
      });
    } finally {
      setAnalyzing(false);
    }
  }, [analyzing, loadMeetings, selected]);

  useEffect(() => {
    if (
      tab === "intelligence" &&
      selected &&
      !selected.intelligence &&
      analysisAttemptedRef.current !== selected.id
    ) {
      analysisAttemptedRef.current = selected.id;
      void analyze();
    }
  }, [analyze, selected, tab]);

  const exportMeeting = async () => {
    if (!selected) return;
    try {
      const markdown = await invoke<string>("export_meeting_markdown", {
        id: selected.id,
      });
      const path = await save({
        defaultPath: `${selected.title.replace(/[^a-z0-9 -]/gi, "").trim() || "Meeting"}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await writeTextFile(path, markdown);
      toast.success("Meeting exported");
    } catch (error) {
      toast.error("Export failed", { description: String(error) });
    }
  };

  const ask = async () => {
    if (!selected || !question.trim() || asking) return;
    setAsking(true);
    try {
      setAnswer(
        await invoke<string>("ask_meeting", {
          id: selected.id,
          question: question.trim(),
        }),
      );
    } catch (error) {
      toast.error("Question failed", { description: String(error) });
    } finally {
      setAsking(false);
    }
  };

  const visibleTranscript = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!selected || !query) return selected?.transcript ?? [];
    return selected.transcript.filter(
      (segment) =>
        segment.text.toLowerCase().includes(query) ||
        segment.speaker.toLowerCase().includes(query),
    );
  }, [search, selected]);

  const visibleMeetings = useMemo(() => {
    const query = meetingSearch.trim().toLowerCase();
    if (!query) return meetings;
    return meetings.filter((meeting) =>
      meeting.title.toLowerCase().includes(query),
    );
  }, [meetingSearch, meetings]);

  const openTranscriptAt = (startMilliseconds: number) => {
    setTab("transcript");
    window.setTimeout(() => {
      document
        .getElementById(`meeting-segment-${startMilliseconds}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 0);
  };

  if (!selected) {
    return (
      <main className="mx-auto w-full max-w-3xl px-4 pb-16 pt-3">
        <header className="mb-8">
          <h1 className="text-xl font-medium tracking-tight text-text">
            {t("meeting.title")}
          </h1>
          <label
            className={`mt-5 flex h-10 w-full items-center gap-2.5 rounded-[8px] px-3 text-stone-500 transition-colors duration-150 ${isLight ? "bg-stone-100 hover:bg-stone-200 focus-within:bg-stone-200" : "bg-[#363230] hover:bg-[#3F3B37] focus-within:bg-[#3F3B37]"}`}
          >
            <HugeiconsIcon icon={Search01Icon} size={16} aria-hidden="true" />
            <input
              value={meetingSearch}
              onChange={(event) => setMeetingSearch(event.target.value)}
              placeholder="Search meetings"
              className="min-w-0 flex-1 bg-transparent text-[13px] text-text outline-none placeholder:text-stone-500"
            />
          </label>
        </header>

        {loading ? (
          <div className="space-y-2" aria-label="Loading meetings">
            {[0, 1, 2].map((item) => (
              <div
                key={item}
                className="h-[68px] animate-pulse rounded-xl bg-surface"
              />
            ))}
          </div>
        ) : meetings.length === 0 ? (
          <div className="flex min-h-72 flex-col items-center justify-center text-center">
            <HugeiconsIcon
              icon={File02Icon}
              size={24}
              className="text-stone-500"
              aria-hidden="true"
            />
            <h2 className="mt-4 text-[15px] font-medium text-text">
              {t("meeting.emptyTitle")}
            </h2>
            <p className="mt-1 max-w-sm text-[13px] leading-5 text-stone-500">
              {t("meeting.emptyDescription")}
            </p>
          </div>
        ) : (
          <div
            className={`overflow-hidden rounded-[10px] bg-surface ${isLight ? "border border-stone-200/60" : ""}`}
          >
            {visibleMeetings.map((meeting, index) => (
              <div
                key={meeting.id}
                className={`group flex items-center pr-3 transition-colors duration-150 ${isLight ? "hover:bg-surface-hover" : "hover:bg-[#363230]"} ${index > 0 ? "border-t border-divider/50" : ""}`}
              >
                <button
                  type="button"
                  onClick={() => void openMeeting(meeting.id)}
                  className="flex min-w-0 flex-1 items-center gap-3 px-4 py-3.5 text-left focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-blue-500"
                >
                  <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background text-stone-500">
                    <HugeiconsIcon
                      icon={File02Icon}
                      size={16}
                      aria-hidden="true"
                    />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[14px] font-medium text-text">
                      {meeting.title}
                    </span>
                    <span className="mt-0.5 block text-[12px] text-stone-500">
                      {formatMeetingDate(meeting.started_at)} ·{" "}
                      {formatDuration(meeting.duration_ms)}
                    </span>
                  </span>
                </button>
                {meeting.has_intelligence ? (
                  <Badge variant="green" className="text-[11px]">
                    {t("meeting.intelligenceReady")}
                  </Badge>
                ) : (
                  <button
                    type="button"
                    onClick={() => void openMeeting(meeting.id, "intelligence")}
                    className={`h-8 cursor-pointer px-2 text-[12px] font-medium underline underline-offset-4 transition-colors disabled:cursor-not-allowed disabled:opacity-45 ${isLight ? "text-stone-500 decoration-stone-500/50 hover:text-stone-900 hover:decoration-stone-900" : "text-stone-300 decoration-stone-300/60 hover:text-stone-50 hover:decoration-stone-50"}`}
                  >
                    {t("meeting.generateIntelligence")}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </main>
    );
  }

  const intelligence = selected.intelligence;
  const speakers = new Set(
    selected.transcript.map((segment) => segment.speaker),
  ).size;
  // Deterministic per-meeting speaker colors: first speaker (you) neutral,
  // second orange, remaining speakers cycle a multi-color palette.
  const speakerVariants = useMemo(() => {
    const palette: BadgeVariant[] = [
      "blue",
      "violet",
      "cyan",
      "purple",
      "sky",
      "fuchsia",
      "indigo",
      "pink",
      "yellow",
      "rose",
    ];
    const order = Array.from(
      new Set(selected.transcript.map((segment) => segment.speaker)),
    );
    return new Map(
      order.map((speaker, index): [string, BadgeVariant] => [
        speaker,
        index === 0
          ? "neutral"
          : index === 1
            ? "orange"
            : palette[(index - 2) % palette.length],
      ]),
    );
  }, [selected]);

  return (
    <main className="mx-auto flex min-h-full w-full max-w-4xl flex-col px-6 pb-16 pt-3">
      <button
        type="button"
        onClick={() => setSelected(null)}
        className={`${actionClass} -ml-2 w-fit px-2`}
      >
        <HugeiconsIcon icon={ArrowLeft01Icon} size={15} aria-hidden="true" />{" "}
        {t("meeting.back")}
      </button>
      <header className="mt-5">
        <div className="flex items-start justify-between gap-6">
          <div className="min-w-0">
            <h1 className="truncate text-xl font-medium tracking-tight text-text">
              {selected.title}
            </h1>
            <p className="mt-1 text-[13px] text-stone-500">
              {formatMeetingDate(selected.started_at)} ·{" "}
              {formatDuration(selected.duration_ms)}
            </p>
          </div>
          <Button
            role="button"
            tabIndex={0}
            variant="secondary"
            size="sm"
            onClick={() => void exportMeeting()}
            icon={
              <HugeiconsIcon
                icon={Download01Icon}
                size={14}
                aria-hidden="true"
              />
            }
          >
            {t("meeting.export")}
          </Button>
        </div>
        <div
          className="mt-7 flex w-fit items-center gap-1 overflow-x-auto border-b border-divider/60"
          role="tablist"
        >
          {tabs.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              onClick={() => setTab(item.id)}
              className={`cursor-pointer rounded-[6px] px-3 py-1.5 text-[13px] font-medium transition-colors duration-150 ${tab === item.id ? (isLight ? "bg-stone-200 text-stone-900" : "bg-[#393532] text-stone-50") : isLight ? "text-stone-500 hover:bg-stone-100 hover:text-stone-900" : "text-stone-400 hover:bg-[#363230] hover:text-stone-50"}`}
            >
              {item.label}
            </button>
          ))}
        </div>
      </header>

      {tab === "transcript" && (
        <section className="pt-7">
          <div className="mb-8 flex items-center justify-between gap-4">
            <p className="text-[12px] text-stone-500">
              {t("meeting.transcriptMeta", {
                speakers,
                segments: selected.transcript.length,
              })}
            </p>
            <label
              className={`flex h-9 w-60 items-center gap-2 rounded-[8px] px-3 text-stone-500 transition-colors duration-150 focus-within:text-text ${isLight ? "bg-stone-100 hover:bg-stone-200 focus-within:bg-stone-200" : "bg-[#363230] hover:bg-[#3F3B37] focus-within:bg-[#3F3B37]"}`}
            >
              <HugeiconsIcon icon={Search01Icon} size={14} aria-hidden="true" />
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search transcript"
                className="min-w-0 flex-1 bg-transparent text-[12px] text-text outline-none placeholder:text-stone-500"
              />
            </label>
          </div>
          <div className="mx-auto max-w-2xl select-text">
            {visibleTranscript.map((segment, index) => (
              <article
                id={`meeting-segment-${segment.start_ms}`}
                key={`${segment.start_ms}-${index}`}
                className="scroll-mt-8 pb-9"
              >
                <div className="mb-3 flex items-center justify-between">
                  <Badge
                    variant={speakerVariants.get(segment.speaker) ?? "neutral"}
                  >
                    {segment.speaker}
                  </Badge>
                  <span className="font-mono text-[11px] text-stone-500">
                    {formatTimestamp(segment.start_ms)}
                  </span>
                </div>
                <p className="whitespace-pre-wrap text-[15px] leading-7 text-text/90">
                  {segment.text}
                </p>
              </article>
            ))}
          </div>
        </section>
      )}

      {tab === "intelligence" && (
        <section className="mx-auto w-full max-w-2xl select-text pt-8">
          {analyzing && !intelligence ? (
            <div
              className="space-y-7"
              aria-label="Generating meeting intelligence"
            >
              {["w-2/3", "w-full", "w-5/6", "w-3/4"].map((width) => (
                <div key={width} className="space-y-3">
                  <div
                    className={`h-4 ${width} animate-pulse rounded bg-surface`}
                  />
                  <div className="h-3 w-full animate-pulse rounded bg-surface" />
                  <div className="h-3 w-4/5 animate-pulse rounded bg-surface" />
                </div>
              ))}
            </div>
          ) : intelligence ? (
            <div className="divide-y divide-divider/60">
              {intelligence.outcome && (
                <section className="pb-6">
                  <div className="mb-3 flex items-center gap-2">
                    <h3 className="text-[15px] font-medium text-text">
                      {t("meeting.sections.outcome")}
                    </h3>
                    <Badge variant="neutral" className="text-[11px]">
                      {intelligence.meeting_type.replace(/_/g, " ")}
                    </Badge>
                  </div>
                  <p className="text-[14px] leading-6 text-stone-500">
                    {intelligence.outcome}
                  </p>
                </section>
              )}
              {intelligence.what_went_well.length > 0 && (
                <section className="py-6">
                  <h3 className="mb-4 text-[15px] font-medium text-text">
                    {t("meeting.sections.whatWorked")}
                  </h3>
                  {intelligence.what_went_well.map((item) => (
                    <IntelligenceFinding
                      key={`${item.issue}-${item.timestamp}`}
                      item={item}
                      label="Strong"
                      variant="green"
                    />
                  ))}
                </section>
              )}
              {intelligence.mistakes.length > 0 && (
                <section className="py-6">
                  <h3 className="mb-4 text-[15px] font-medium text-text">
                    {t("meeting.sections.lostGround")}
                  </h3>
                  {intelligence.mistakes.map((item) => (
                    <IntelligenceFinding
                      key={`${item.issue}-${item.timestamp}`}
                      item={item}
                      label="Mistake"
                      variant="rose"
                    />
                  ))}
                </section>
              )}
              {intelligence.missed_opportunities.length > 0 && (
                <section className="py-6">
                  <h3 className="mb-4 text-[15px] font-medium text-text">
                    {t("meeting.sections.missedOpportunities")}
                  </h3>
                  {intelligence.missed_opportunities.map((item) => (
                    <IntelligenceFinding
                      key={`${item.issue}-${item.timestamp}`}
                      item={item}
                      label="Opportunity"
                      variant="yellow"
                    />
                  ))}
                </section>
              )}
              {intelligence.communication_issues.length > 0 && (
                <section className="py-6">
                  <h3 className="mb-4 text-[15px] font-medium text-text">
                    {t("meeting.sections.communication")}
                  </h3>
                  {intelligence.communication_issues.map((item) => (
                    <IntelligenceFinding
                      key={`${item.issue}-${item.timestamp}`}
                      item={item}
                      label="Communication"
                      variant="orange"
                    />
                  ))}
                </section>
              )}
              <TextSection
                title={t("meeting.sections.decisions")}
                items={intelligence.important_decisions}
              />
              <TextSection
                title={t("meeting.sections.actions")}
                items={intelligence.action_items}
              />
              <TextSection
                title={t("meeting.sections.risks")}
                items={intelligence.risks}
              />
              <TextSection
                title={t("meeting.sections.lessons")}
                items={intelligence.lessons}
              />
              <TextSection
                title={t("meeting.sections.nextMeeting")}
                items={intelligence.next_time}
              />
            </div>
          ) : (
            <div className="py-20 text-center">
              <p className="text-[13px] text-stone-500">
                {t("meeting.intelligenceUnavailable")}
              </p>
              <button
                type="button"
                onClick={() => void analyze()}
                className={`${actionClass} mt-3`}
              >
                {t("meeting.tryAgain")}
              </button>
            </div>
          )}
        </section>
      )}

      {tab === "ask" && (
        <section className="mx-auto w-full max-w-2xl pt-6">
          <div className="select-text">
            {answer ? (
              <div className="text-[14px] leading-7 text-text/90">
                <MeetingAnswer
                  answer={answer}
                  transcript={selected.transcript}
                  onTimestamp={openTranscriptAt}
                />
              </div>
            ) : (
              <div className="text-[13px] text-stone-500">
                {t("meeting.askEmpty")}
              </div>
            )}
          </div>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void ask();
            }}
            className={`mt-5 flex items-end gap-2 rounded-[10px] border border-transparent p-2 transition-colors duration-150 ${isLight ? "bg-stone-100 hover:border-stone-300 focus-within:border-transparent focus-within:bg-stone-200" : "bg-[#363230] hover:border-stone-700 focus-within:border-transparent focus-within:bg-[#393532]"}`}
          >
            <textarea
              value={question}
              onChange={(event) => setQuestion(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  void ask();
                }
              }}
              rows={1}
              placeholder="Ask anything about this meeting…"
              className="max-h-32 min-h-9 flex-1 resize-none bg-transparent px-2 py-2 text-[13px] leading-5 text-text outline-none placeholder:text-stone-500"
            />
            <button
              type="submit"
              disabled={!question.trim() || asking}
              aria-label="Ask meeting"
              className="flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-[6px] bg-blue-600 text-white transition-colors duration-150 hover:bg-blue-700 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40"
            >
              <HugeiconsIcon
                icon={ArrowUp02Icon}
                size={15}
                aria-hidden="true"
              />
            </button>
          </form>
        </section>
      )}
    </main>
  );
}
