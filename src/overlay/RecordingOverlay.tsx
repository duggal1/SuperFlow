import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUp, Check, Copy } from "@phosphor-icons/react";
import { HugeiconsIcon } from "@hugeicons/react";
import { SquareIcon } from "@hugeicons/core-free-icons";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import "./RecordingOverlay.css";
import { commands, events } from "@/bindings";
import { Badge, type BadgeVariant } from "@/components/ui/Badge";
import type {
  StreamPhase,
  StreamPhaseEvent,
  StreamTextEvent,
  StreamWorkKind,
} from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { Button } from "@/components/ui/Button";
import { LiveWaveform } from "@/components/waves/live-waveform";
import { IOSSpinner } from "@/components/shared/global-spinner";

type OverlayState =
  | "recording"
  | "hands_free"
  | "meeting"
  | "meeting_transcribing"
  | "meeting_saved"
  | "streaming"
  | "transcribing"
  | "processing"
  | "prompting"
  | "editing"
  | "say_this"
  | "calendar_processing"
  | "calendar_success"
  | "calendar_clarify"
  | "calendar_failure"
  | "ai_notice";

interface CalendarSuccessPayload {
  ok: boolean;
  action: string;
  title: string;
  start: string;
  end: string;
  calendar: string;
  event_id: string;
  success_message: string;
}

export const LOADING_STATES: readonly string[] = [
  "Recombobulating",
  "Cooking",
  "Percolating",
  "Tinkering",
  "Orchestrating",
  "Brewing",
  "Synthesizing",
  "Noodling",
  "Wrangling",
  "Whirring",
] as const;

interface AiCleanupNotice {
  message: string;
  badge: string;
  variant: "error" | "warning" | "success";
}

const NOTICE_BADGE_VARIANTS: Record<AiCleanupNotice["variant"], BadgeVariant> =
  {
    error: "rose",
    warning: "orange",
    success: "green",
  };

// The backend emits 16 FFT buckets. Every overlay form feeds the same smoothed
// values into the shared LiveWaveform component.
const WAVE_BUCKETS = 16;

// How long the result card plays its blur-out before React tears it down. The
// backend hides the native panel ~300ms after emitting "hide-overlay"; the
// exit must land inside that grace window.
const RESULT_EXIT_MS = 240;

// Which edges of the result-card body get the mask fade: whenever the
// transcript overflows the scroll cap, BOTH edges fade — top and bottom.
type ResultFade = "none" | "faded";

// Measures whether the transcript body is hiding scrolled text. Module-level
// so both the layout effect and the scroll handler share one implementation.
const measureResultFade = (
  el: HTMLDivElement | null,
  setFade: (fade: ResultFade) => void,
) => {
  setFade(el && el.scrollHeight > el.clientHeight + 1 ? "faded" : "none");
};

// ---- Dialog motion (result card + cancel toast) ---------------------------
// Spring-driven transforms with eased opacity/blur: the physical settle of a
// spring reads far smoother than one fixed-duration tween, while the blur
// stays on a tween so it never wobbles around its target. The drift direction
// follows the overlay's screen edge (rises from the bottom edge, drops from
// the top edge).
const DIALOG_EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];
const dialogTransition = {
  type: "spring" as const,
  stiffness: 350,
  damping: 30,
  mass: 0.92,
  opacity: { duration: 0.26, ease: DIALOG_EASE },
  filter: { duration: 0.32, ease: DIALOG_EASE },
};
// Exit keeps everything on short tweens — springs settling during teardown
// would fight the native window hide.
const dialogExitTransition = {
  opacity: { duration: 0.2, ease: DIALOG_EASE },
  filter: { duration: 0.24, ease: DIALOG_EASE },
};

const dialogEnter = (driftY: number) => ({
  opacity: 0,
  scale: 0.96,
  y: driftY,
  filter: "blur(6px)",
});
const dialogShown = {
  opacity: 1,
  scale: 1,
  y: 0,
  filter: "blur(0px)",
};
const dialogExit = (driftY: number) => ({
  opacity: 0,
  scale: 0.98,
  y: driftY * 0.5,
  filter: "blur(5px)",
});

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  // `Stream::play()` returning does not mean hardware callbacks are flowing.
  // Stay visually in an arming state until the backend processes the first
  // actual microphone sample chunk.
  const [captureReady, setCaptureReady] = useState(false);
  const [levels, setLevels] = useState<number[]>(Array(WAVE_BUCKETS).fill(0));
  const [streamText, setStreamText] = useState<StreamTextEvent>({
    committed: "",
    tentative: "",
  });
  const [phase, setPhase] = useState<StreamPhase>("listening");
  const [workKind, setWorkKind] = useState<StreamWorkKind>("transcribing");
  const [elapsed, setElapsed] = useState(0);
  // Bumped on each new streaming session so the Live card remounts fresh (replays
  // the pop-in, and never animates in from the previous panel's open size).
  const [session, setSession] = useState(0);
  // Overlay placement (top vs bottom of the screen). The Live panel grows downward
  // from a top overlay (oldest line under the pill) and upward from a bottom one.
  const [position, setPosition] = useState<"top" | "bottom">("bottom");
  // True once live text overflows the cap. A top overlay fades its top edge only
  // while overflowing, so the resting first line stays crisp flush under the pill.
  const [overflowing, setOverflowing] = useState(false);
  // Transcript result card: non-null while the finished dictation is on screen
  // with its copy affordance. Driven by "show-transcript-result" from Rust.
  const [resultText, setResultText] = useState<string | null>(null);
  // True right after a successful copy — swaps the button to "Copied" before
  // the card dismisses itself.
  const [copied, setCopied] = useState(false);
  // True while the card plays its blur-out exit (between "hide-overlay" and
  // the native panel hiding itself).
  const [resultExiting, setResultExiting] = useState(false);
  // Which edges of the body are currently hiding scrolled text.
  const [resultFade, setResultFade] = useState<ResultFade>("none");
  // Cancel acknowledgment toast: shown when a dictation is cancelled, with
  // Undo enabled once the pipeline stashes the cancelled transcript.
  const [cancelToastVisible, setCancelToastVisible] = useState(false);
  const [cancelToastCanUndo, setCancelToastCanUndo] = useState(false);
  const [cancelToastExiting, setCancelToastExiting] = useState(false);
  const [aiCleanupNotice, setAiCleanupNotice] =
    useState<AiCleanupNotice | null>(null);
  // Dedicated AI "Say this" pill: random loading state per invocation,
  // stays fixed for that run (not animating cycle). Single spinner left only — extremely clean.
  const [sayThisLabel, setSayThisLabel] = useState<string>(LOADING_STATES[0]);
  // Standard (non-AI) loading also uses random LOADING_STATES, not "Transcribing..."
  const [standardLoadingLabel, setStandardLoadingLabel] = useState<string>(
    LOADING_STATES[0],
  );
  // Calendar result states (reuse pill architecture, backend is source of truth)
  const [calendarSuccess, setCalendarSuccess] =
    useState<CalendarSuccessPayload | null>(null);
  const [calendarClarify, setCalendarClarify] = useState<string | null>(null);
  const [calendarFailure, setCalendarFailure] = useState<string | null>(null);
  const [calendarProcessingTitle, setCalendarProcessingTitle] =
    useState<string>("");
  // Clarification input — when AI asks "What date do you want me to set?" we render an input
  const [clarifyInput, setClarifyInput] = useState("");
  const [pendingCalendarTranscript, setPendingCalendarTranscript] = useState<
    string | null
  >(null);
  // Auto-dismiss safety net so the floating card can never linger forever.
  const resultTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Delayed close after the "Copied" confirmation plays.
  const copyCloseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Blur-out teardown timer (see RESULT_EXIT_MS).
  const resultExitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Mirror of resultText for event listeners registered once on mount.
  const resultTextRef = useRef<string | null>(null);
  // Scrollable transcript body — measured for the edge fades.
  const resultBodyRef = useRef<HTMLDivElement>(null);
  // Cancel-toast timers + visibility mirror for the same listeners.
  const cancelToastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const cancelToastExitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const cancelToastVisibleRef = useRef(false);
  const aiNoticeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Refs mirroring live state so the once-registered show-overlay listener can
  // detect a promote (recording/streaming → hands_free) without a stale closure.
  const stateRef = useRef<OverlayState>("recording");
  const captureReadyRef = useRef(false);
  useEffect(() => {
    stateRef.current = state;
  }, [state]);
  useEffect(() => {
    captureReadyRef.current = captureReady;
  }, [captureReady]);

  const clearResultTimer = () => {
    if (resultTimerRef.current !== null) {
      clearTimeout(resultTimerRef.current);
      resultTimerRef.current = null;
    }
  };

  const clearCopyCloseTimer = () => {
    if (copyCloseTimerRef.current !== null) {
      clearTimeout(copyCloseTimerRef.current);
      copyCloseTimerRef.current = null;
    }
  };

  const clearResultExitTimer = () => {
    if (resultExitTimerRef.current !== null) {
      clearTimeout(resultExitTimerRef.current);
      resultExitTimerRef.current = null;
    }
  };

  const resetResultCard = () => {
    clearResultTimer();
    clearCopyCloseTimer();
    clearResultExitTimer();
    resultTextRef.current = null;
    setResultText(null);
    setCopied(false);
    setResultExiting(false);
  };

  // Plays the card's blur-out, then tears the state down. Runs inside the
  // backend's ~300ms grace between "hide-overlay" and the native hide.
  const startResultExit = () => {
    clearResultTimer();
    clearCopyCloseTimer();
    setResultExiting(true);
    resultExitTimerRef.current = setTimeout(() => {
      resultExitTimerRef.current = null;
      resultTextRef.current = null;
      setResultExiting(false);
      setResultText(null);
      setCopied(false);
      setIsVisible(false);
    }, RESULT_EXIT_MS);
  };

  const clearCancelToastTimers = () => {
    if (cancelToastTimerRef.current !== null) {
      clearTimeout(cancelToastTimerRef.current);
      cancelToastTimerRef.current = null;
    }
    if (cancelToastExitTimerRef.current !== null) {
      clearTimeout(cancelToastExitTimerRef.current);
      cancelToastExitTimerRef.current = null;
    }
  };

  const resetCancelToast = () => {
    clearCancelToastTimers();
    cancelToastVisibleRef.current = false;
    setCancelToastVisible(false);
    setCancelToastExiting(false);
    setCancelToastCanUndo(false);
  };

  // Plays the toast's blur-out, then tears the state down — same backend
  // grace window as the result card's exit.
  const startCancelToastExit = () => {
    clearCancelToastTimers();
    setCancelToastExiting(true);
    cancelToastExitTimerRef.current = setTimeout(() => {
      cancelToastExitTimerRef.current = null;
      cancelToastVisibleRef.current = false;
      setCancelToastExiting(false);
      setCancelToastVisible(false);
      setCancelToastCanUndo(false);
      setIsVisible(false);
    }, RESULT_EXIT_MS);
  };

  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  // Live-text scroll-back: the text region "sticks" to the newest line while the
  // user is at the bottom; if they scroll up to read history, auto-follow pauses
  // until they scroll back down.
  const capRef = useRef<HTMLDivElement>(null);
  const pinnedRef = useRef(true);
  const direction = getLanguageDirection(i18n.language);

  useEffect(() => {
    const setupEventListeners = async () => {
      const unlistenShow = await listen("show-overlay", async (event) => {
        // "result" rides the same resize path as the capture states but the
        // result card itself is driven solely by "show-transcript-result".
        const overlayState = event.payload as OverlayState | "result";
        if (overlayState === "result") return;
        // Promote (recording/streaming → hands_free) keeps the live timer and
        // capture state so the pill continues from e.g. 00:31 instead of
        // resetting to 00:00. All other show-overlay paths are fresh dictations.
        const isHandsFreePromote =
          overlayState === "hands_free" &&
          (stateRef.current === "recording" ||
            stateRef.current === "streaming") &&
          captureReadyRef.current;
        if (isHandsFreePromote) {
          await syncLanguageFromSettings();
          try {
            const settings = await commands.getAppSettings();
            if (settings.status === "ok") {
              setPosition(
                settings.data.overlay_position === "top" ? "top" : "bottom",
              );
            }
          } catch {
            // Keep the previous/default placement if settings can't be read.
          }
          setState(overlayState);
          setIsVisible(true);
          return;
        }
        // Reset synchronously before settings I/O. A fast microphone can emit
        // recording-ready while the awaits below are in flight; resetting after
        // them would overwrite that event and leave the overlay stuck arming.
        if (
          overlayState === "recording" ||
          overlayState === "hands_free" ||
          overlayState === "meeting" ||
          overlayState === "streaming"
        ) {
          setCaptureReady(false);
          smoothedLevelsRef.current = Array(16).fill(0);
          setLevels(Array(WAVE_BUCKETS).fill(0));
          setStreamText({ committed: "", tentative: "" });
          // A new dictation replaces any result card or cancel toast.
          resetResultCard();
          resetCancelToast();
          setAiCleanupNotice(null);
        }

        await syncLanguageFromSettings();
        // The Live panel flows downward from a top overlay and upward from a
        // bottom one; read the placement so the layout can flip to match.
        try {
          const settings = await commands.getAppSettings();
          if (settings.status === "ok") {
            setPosition(
              settings.data.overlay_position === "top" ? "top" : "bottom",
            );
          }
        } catch {
          // Keep the previous/default placement if settings can't be read.
        }
        // For AI "Say this" pill, pick one random LOADING_STATES per invocation
        // and keep it fixed for that run (not cycling). Each click gets a new random.
        // Single spinner left only — extremely clean.
        if (overlayState === "say_this") {
          const pick =
            LOADING_STATES[Math.floor(Math.random() * LOADING_STATES.length)];
          setSayThisLabel(pick);
        }
        // Standard loading (non-AI) also uses random LOADING_STATES, not "Transcribing..."
        if (overlayState === "transcribing" || overlayState === "processing") {
          const pick =
            LOADING_STATES[Math.floor(Math.random() * LOADING_STATES.length)];
          setStandardLoadingLabel(pick);
        }
        setState(overlayState);
        if (overlayState === "streaming") {
          setPhase("listening");
          setWorkKind("transcribing");
          setElapsed(0);
          setSession((s) => s + 1); // remount the card fresh for this session
        }
        setIsVisible(true);
      });

      const unlistenHide = await listen("hide-overlay", () => {
        setCaptureReady(false);
        if (cancelToastVisibleRef.current) {
          // Cancel toast on screen: blur it out inside the backend's grace.
          if (cancelToastExitTimerRef.current === null) {
            startCancelToastExit();
          }
          return;
        }
        if (resultTextRef.current === null) {
          setIsVisible(false);
          resetResultCard();
          return;
        }
        // The result card is on screen: play its blur-out inside the backend's
        // ~300ms grace before the native panel hides, then tear down state.
        if (resultExitTimerRef.current === null) {
          startResultExit();
        }
      });

      const unlistenCancelToast = await listen<boolean>(
        "show-cancel-toast",
        (event) => {
          clearCancelToastTimers();
          resetResultCard();
          cancelToastVisibleRef.current = true;
          setCancelToastVisible(true);
          setCancelToastExiting(false);
          setCancelToastCanUndo(event.payload);
          setIsVisible(false); // only the toast renders
          cancelToastTimerRef.current = setTimeout(() => {
            cancelToastTimerRef.current = null;
            void commands.hideResultOverlay();
          }, 3_000);
        },
      );

      const unlistenResult = await listen<string>(
        "show-transcript-result",
        (event) => {
          clearResultTimer();
          clearCopyCloseTimer();
          clearResultExitTimer();
          resultTextRef.current = event.payload;
          setResultText(event.payload);
          setCopied(false);
          setResultExiting(false);
          setIsVisible(true);
          resultTimerRef.current = setTimeout(() => {
            resultTimerRef.current = null;
            void commands.hideResultOverlay();
          }, 10_000);
        },
      );

      const unlistenAiNotice = await listen<AiCleanupNotice>(
        "show-ai-cleanup-notice",
        (event) => {
          if (aiNoticeTimerRef.current !== null) {
            clearTimeout(aiNoticeTimerRef.current);
          }
          setAiCleanupNotice(event.payload);
          setIsVisible(true);
          aiNoticeTimerRef.current = setTimeout(() => {
            aiNoticeTimerRef.current = null;
            setAiCleanupNotice(null);
            void commands.hideResultOverlay();
          }, 3_500);
        },
      );

      const unlistenReady = await listen("recording-ready", () => {
        setElapsed(0);
        setCaptureReady(true);
      });

      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];
        // Exponential smoothing across the backend's 16 FFT buckets.
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed);
      });

      const unlistenStream = await events.streamTextEvent.listen((event) => {
        setStreamText(event.payload);
      });

      const unlistenPhase = await events.streamPhaseEvent.listen((event) => {
        const payload: StreamPhaseEvent = event.payload;
        setPhase(payload.phase);
        if (payload.kind) setWorkKind(payload.kind);
      });

      const unlistenCalendarProcessing = await listen<string>(
        "calendar-processing",
        (event) => {
          setCalendarProcessingTitle(event.payload);
          setPendingCalendarTranscript(event.payload);
        },
      );
      const unlistenCalendarSuccess = await listen<CalendarSuccessPayload>(
        "calendar-success",
        (event) => {
          setCalendarSuccess(event.payload);
          setCalendarClarify(null);
          setCalendarFailure(null);
          setPendingCalendarTranscript(null);
          setClarifyInput("");
        },
      );
      const unlistenCalendarClarify = await listen<string>(
        "calendar-clarify",
        (event) => {
          setCalendarClarify(event.payload);
          setCalendarSuccess(null);
          setCalendarFailure(null);
          // Keep pending transcript for when user answers; if not set, use the clarify question's context
          // The processing title already holds the original transcript
        },
      );
      const unlistenCalendarFailure = await listen<string>(
        "calendar-failure",
        (event) => {
          setCalendarFailure(event.payload);
          setCalendarSuccess(null);
          setCalendarClarify(null);
          setPendingCalendarTranscript(null);
          setClarifyInput("");
        },
      );

      return () => {
        unlistenShow();
        unlistenHide();
        unlistenReady();
        unlistenLevel();
        unlistenStream();
        unlistenPhase();
        unlistenCalendarProcessing();
        unlistenCalendarSuccess();
        unlistenCalendarClarify();
        unlistenCalendarFailure();
        unlistenResult();
        unlistenCancelToast();
        unlistenAiNotice();
        if (aiNoticeTimerRef.current !== null) {
          clearTimeout(aiNoticeTimerRef.current);
        }
      };
    };

    setupEventListeners();
  }, []);

  // Elapsed capture timer starts only once microphone samples are flowing.
  // Shows for hands_free, streaming, and standard recording (logo | waveform | timer)
  useEffect(() => {
    if (
      (state !== "streaming" &&
        state !== "hands_free" &&
        state !== "meeting" &&
        state !== "recording") ||
      !isVisible ||
      !captureReady
    )
      return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [state, isVisible, captureReady]);

  // Stick to the bottom as text streams in — but only while pinned, so a user who
  // has scrolled up to read history isn't yanked back down by the next chunk.
  useLayoutEffect(() => {
    const el = capRef.current;
    if (!el) return;
    // Fade the top edge only once text actually overflows the cap.
    setOverflowing(el.scrollHeight > el.clientHeight + 1);
    if (pinnedRef.current) el.scrollTop = el.scrollHeight;
  }, [streamText]);

  // Each fresh streaming session starts pinned to the bottom, fade cleared.
  useEffect(() => {
    pinnedRef.current = true;
    setOverflowing(false);
  }, [session]);

  // Result-card edge fades depend on whether the finished transcript actually
  // overflows the scroll cap, and on which edges are hiding scrolled text.
  // Re-measured whenever a new card arrives; scrolling re-measures live.
  useLayoutEffect(() => {
    if (!resultText) {
      setResultFade("none");
      return;
    }
    measureResultFade(resultBodyRef.current, setResultFade);
  }, [resultText]);

  if (!isVisible && !cancelToastVisible && !aiCleanupNotice) return null;

  if (aiCleanupNotice) {
    const driftY = position === "top" ? -10 : 10;
    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <motion.div
          className={`scard sai-notice ${aiCleanupNotice.variant}`}
          initial={dialogEnter(driftY)}
          animate={{ ...dialogShown, transition: dialogTransition }}
        >
          <span className="sai-notice-label">{aiCleanupNotice.message}</span>
          <Badge
            variant={NOTICE_BADGE_VARIANTS[aiCleanupNotice.variant]}
            className="rounded-[7px] text-[12px]"
          >
            {aiCleanupNotice.badge}
          </Badge>
        </motion.div>
      </div>
    );
  }

  // ---- Cancel acknowledgment toast ----
  // "Transcription canceled" (+ Undo once a transcript is stashed backend-
  // side). Auto-dismisses after 3s; Undo re-pastes through the normal path.
  const handleUndoCancel = async () => {
    if (!cancelToastCanUndo) return;
    setCancelToastCanUndo(false);
    clearCancelToastTimers();
    try {
      await commands.undoCanceledTranscription();
    } catch (err) {
      console.error("Failed to undo canceled transcription:", err);
    }
  };

  if (cancelToastVisible) {
    // Drift direction follows the screen edge the overlay is anchored to.
    const driftY = position === "top" ? -10 : 10;
    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <motion.div
          className="scard scancel"
          initial={dialogEnter(driftY)}
          animate={
            cancelToastExiting
              ? {
                  ...dialogExit(driftY),
                  transition: dialogExitTransition,
                }
              : { ...dialogShown, transition: dialogTransition }
          }
        >
          <span className="scancel-label">{t("overlay.transcript")}</span>
          <div className="scancel-actions">
            <Badge variant="rose" className="rounded-[7px] text-[12px]">
              {t("overlay.canceled")}
            </Badge>
            {cancelToastCanUndo && (
              <Button
                variant="secondary"
                size="sm"
                role="button"
                tabIndex={0}
                className="rounded-lg"
                onClick={() => void handleUndoCancel()}
              >
                {t("overlay.undo")}
              </Button>
            )}
          </div>
        </motion.div>
      </div>
    );
  }

  // ---- Transcript result card ----
  // Copy puts the full finished dictation on the clipboard, plays the "Copied"
  // confirmation on the button, then dismisses the card 500ms later — copying
  // is dismissal, but the confirmation always gets to land first.
  const handleCopy = async () => {
    if (!resultText || copied) return;
    try {
      // The overlay panel never becomes the key window, so navigator.clipboard
      // is unreliable here; the Tauri plugin writes via the Rust side instead.
      await writeText(resultText);
    } catch (err) {
      console.error("Failed to copy transcript:", err);
      return;
    }
    setCopied(true);
    clearResultTimer();
    copyCloseTimerRef.current = setTimeout(() => {
      copyCloseTimerRef.current = null;
      closeResult();
    }, 500);
  };
  const closeResult = () => {
    clearResultTimer();
    void commands.hideResultOverlay();
  };

  if (resultText) {
    // The full transcript always renders — Copy uses the untouched resultText
    // and the body scrolls under edge fades when it outgrows the card.
    // Drift direction follows the screen edge the overlay is anchored to.
    const driftY = position === "top" ? -10 : 10;
    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <motion.div
          className="scard sresult"
          initial={dialogEnter(driftY)}
          animate={
            resultExiting
              ? {
                  ...dialogExit(driftY),
                  transition: dialogExitTransition,
                }
              : { ...dialogShown, transition: dialogTransition }
          }
        >
          <div className="sresult-head">
            <span className="sresult-label">{t("overlay.transcript")}</span>
            <motion.button
              type="button"
              onClick={() => void handleCopy()}
              className="group relative inline-flex h-7 cursor-pointer items-center justify-center whitespace-nowrap rounded-[9px] border bg-white px-3.5 py-1 no-underline shadow-none transition-[background,border-color] duration-200 ease-out focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-stone-300 dark:border-stone-700/70 dark:bg-[linear-gradient(#1816131a_0%_100%),linear-gradient(#403c38_0%,#1c1917_100%)] dark:shadow-[0_0_0_1px_#29252426,inset_0_2px_#ffffff24,inset_0_-0.5px_2px_#00000070,0_2px_8px_#0000000a,0_3px_4px_#00000036] dark:hover:border-stone-800 dark:hover:bg-[linear-gradient(#12100e1a_0%_100%),linear-gradient(#312d29_0%,#141210_100%)] dark:hover:shadow-[0_0_0_1px_#1c191733,inset_0_2px_#ffffff18,inset_0_-0.5px_2px_#00000085,0_2px_8px_#00000010,0_3px_4px_#00000042] border-stone-200 hover:border-stone-300 hover:bg-stone-50 dark:focus-visible:ring-stone-600"
              whileTap={{ scale: 0.985 }}
              transition={{
                duration: 0.1,
                ease: [0.22, 1, 0.36, 1],
              }}
            >
              <span
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 rounded-[9px] bg-stone-100 opacity-0 transition-opacity duration-200 ease-out group-hover:opacity-100 dark:bg-stone-950/25"
              />

              <span className="relative z-10 inline-flex items-center justify-center gap-1.5">
                <AnimatePresence initial={false} mode="wait">
                  {copied ? (
                    <motion.span
                      key="copied"
                      initial={{ opacity: 0, y: 2 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -2 }}
                      transition={{
                        duration: 0.14,
                        ease: [0.22, 1, 0.36, 1],
                      }}
                      className="inline-flex items-center justify-center gap-1.5 text-[14px] font-[460] tracking-[0.15px] text-stone-900 dark:text-stone-50"
                    >
                      <Check
                        aria-hidden="true"
                        className="size-3.5"
                        weight="regular"
                      />
                      {t("overlay.copied")}
                    </motion.span>
                  ) : (
                    <motion.span
                      key="copy"
                      initial={{ opacity: 0, y: -2 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: 2 }}
                      transition={{
                        duration: 0.14,
                        ease: [0.22, 1, 0.36, 1],
                      }}
                      className="inline-flex items-center justify-center gap-1.5 text-[13px] font-[460] tracking-[0.15px] text-stone-900 dark:text-stone-50"
                    >
                      <Copy
                        aria-hidden="true"
                        className="size-3.5"
                        weight="regular"
                      />
                      {t("overlay.copy")}
                    </motion.span>
                  )}
                </AnimatePresence>
              </span>
            </motion.button>
          </div>
          {/* Top and bottom edges fade together whenever the transcript
              overflows the scroll cap; short transcripts render fully crisp. */}
          <div
            ref={resultBodyRef}
            onScroll={() =>
              measureResultFade(resultBodyRef.current, setResultFade)
            }
            className={`sresult-body${resultFade === "none" ? "" : " faded"}`}
          >
            {resultText}
          </div>
        </motion.div>
      </div>
    );
  }

  // Re-pin when the user is within ~a line of the bottom; unpin otherwise.
  const handleStreamScroll = () => {
    const el = capRef.current;
    if (!el) return;
    pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight <= 16;
  };

  const fmtTime = (s: number) =>
    `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

  // ---- Shared building blocks (one visual language for every overlay form) ----
  const isLightOverlay =
    typeof document !== "undefined" &&
    (document.documentElement.dataset.theme === "light" ||
      (!document.documentElement.dataset.theme &&
        window.matchMedia("(prefers-color-scheme: light)").matches));
  const waveform = (
    <LiveWaveform
      className="swave"
      active={isVisible}
      levels={levels}
      height={18}
      barWidth={4}
      barGap={3}
      barRadius={2}
      barColor={isLightOverlay ? "#1c1917" : "#fafaf9"}
      fadeEdges={false}
      mode="static"
    />
  );

  const cancelBtn = (
    <button
      className="sx"
      aria-label="cancel"
      onClick={() => commands.cancelOperation()}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path
          d="M4 4 L12 12 M12 4 L4 12"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      </svg>
    </button>
  );

  const completeBtn = (
    <button
      className="scomplete"
      aria-label="Finish transcription"
      onClick={() => commands.completeHandsFreeTranscription()}
    >
      <Check size={13} weight="bold" aria-hidden="true" />
    </button>
  );

  const meetingCompleteBtn = (
    <button
      className="scomplete meeting-stop"
      aria-label="Finish meeting"
      onClick={() => commands.completeHandsFreeTranscription()}
    >
      <HugeiconsIcon icon={SquareIcon} size={11} aria-hidden="true" />
    </button>
  );

  // logo (left) | waveform (center) | timer + cancel (right) — same structure for
  // pill & panel, so the Live morph is a pure width change.
  const listeningRow = (showTimer: boolean, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <img className="slogo" src="/logo.svg" alt="" aria-hidden="true" />
      </div>
      {waveform}
      <div className="sbase-r">
        {showTimer && <span className="stimer">{fmtTime(elapsed)}</span>}
        {showCancel && cancelBtn}
      </div>
    </div>
  );

  // spinner (left) | label (center) | cancel (right) — same 3-zone grid as the
  // listening row, so the label is centered.
  const workingRow = (label: string, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <IOSSpinner size={13} color="var(--s-accent)" speed={1.0} />
      </div>
      <span className="swork-label">{label}</span>
      <div className="sbase-r">{showCancel && cancelBtn}</div>
    </div>
  );

  // AI "Say this" pill: single spinner left only — extremely clean.
  // Random LOADING_STATES label picked once per invocation and stays fixed.
  // Editing remains for edit mode only; AI never shows "Transcription"/"Editing".
  const sayThisRow = (label: string) => (
    <div className="sbase">
      <div className="sbase-l">
        <IOSSpinner size={13} color="var(--s-accent)" speed={1.0} />
      </div>
      <span className="swork-label">{label}...</span>
      <div className="sbase-r" />
    </div>
  );

  const handsFreeRow = (
    <div className="sbase shandsfree">
      {cancelBtn}
      {waveform}
      <span className="stimer">{fmtTime(elapsed)}</span>
      {completeBtn}
    </div>
  );

  const meetingRow = (
    <div className="sbase shandsfree smeeting">
      {waveform}
      <span className="stimer">{fmtTime(elapsed)}</span>
      {meetingCompleteBtn}
    </div>
  );

  if (state === "meeting") {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard compact meeting">{meetingRow}</div>
      </div>
    );
  }

  if (state === "meeting_transcribing") {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard compact meeting working">
          {workingRow(t("overlay.transcribingMeeting"), false)}
        </div>
      </div>
    );
  }

  if (state === "hands_free") {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard compact hands-free">{handsFreeRow}</div>
      </div>
    );
  }

  if (state === "meeting_saved") {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard compact meeting-success">
          <div className="sbase">
            <span className="swork-label">{t("overlay.meetingStatus")}</span>
            <Badge variant="green" className="sbase-r text-[11px]">
              {t("overlay.meetingRecordedSuccessfully")}
            </Badge>
          </div>
        </div>
      </div>
    );
  }

  // ---- Live overlay: a pill that sculpts open into a panel ----
  if (state === "streaming") {
    const hasText =
      streamText.committed.length > 0 || streamText.tentative.length > 0;
    const working = phase === "working";
    // Keep the panel open whenever there's text — even while finalizing — so the
    // transcript stays put under a working spinner instead of collapsing and
    // squishing the text mid-stream. Only fall back to the small working pill
    // when there was no text to preserve.
    const open = hasText;
    const collapsed = working && !hasText;

    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <div
          key={session}
          className={`scard ${open ? "open" : ""} ${collapsed ? "working" : ""} ${
            isVisible ? "" : "leaving"
          }`}
        >
          <div className="stext">
            <div className="stext-clip">
              <div
                className={`stext-cap ${overflowing ? "overflowing" : ""}`}
                ref={capRef}
                onScroll={handleStreamScroll}
              >
                <p>
                  <span className="committed">
                    {streamText.committed ? streamText.committed + " " : ""}
                  </span>
                  <span className="tentative">{streamText.tentative}</span>
                  {/* Drop the blinking caret once finalizing — it's no longer
                      capturing, and a static spinner conveys the work. */}
                  {!working && <span className="scaret" />}
                </p>
              </div>
            </div>
          </div>
          {working
            ? workingRow(
                workKind === "finalizing"
                  ? t("overlay.finalizing", { defaultValue: "Finalizing..." })
                  : workKind === "polishing"
                    ? t("overlay.processing")
                    : t("overlay.transcribing"),
                true,
              )
            : listeningRow(open, true)}
        </div>
      </div>
    );
  }

  // ---- AI "Say this" pill: dedicated dialogue for control-key AI transcription.
  // Uses dual spinners (left + right) and a per-invocation random LOADING_STATES
  // pick that stays fixed for that run. Never shows "Transcription"/"Editing"
  // — those remain reserved for edit mode (Fn + selection via run_edit_mode).
  if (state === "say_this") {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div
          className={`scard compact cworking ai-prompting ${isVisible ? "" : "leaving"}`}
        >
          {sayThisRow(sayThisLabel)}
        </div>
      </div>
    );
  }

  // ---- Calendar states: reuse pill architecture, backend is source of truth ----
  // Only render success after native EventKit save. No fake success.
  const formatCalendarMeta = (
    startStr: string,
    endStr: string,
    calendarName: string,
  ) => {
    try {
      const start = new Date(startStr);
      const end = new Date(endStr);
      const now = new Date();
      const startDay = new Date(
        start.getFullYear(),
        start.getMonth(),
        start.getDate(),
      );
      const nowDay = new Date(now.getFullYear(), now.getMonth(), now.getDate());
      const diffDays = Math.round(
        (startDay.getTime() - nowDay.getTime()) / 86400000,
      );
      const timeFmt = new Intl.DateTimeFormat(undefined, {
        hour: "numeric",
        minute: "2-digit",
      });
      const dateFmt = new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
      });
      const startTime = timeFmt.format(start);
      const endTime = timeFmt.format(end);
      const timeRange =
        startTime === endTime ? startTime : `${startTime}–${endTime}`;
      let dayLabel: string;
      if (diffDays === 0) dayLabel = "Today";
      else if (diffDays === 1) dayLabel = "Tomorrow";
      else dayLabel = dateFmt.format(start);
      const cal =
        calendarName && calendarName !== "Calendar" ? ` · ${calendarName}` : "";
      return `${dayLabel} · ${timeRange}${cal}`;
    } catch {
      return calendarName || "";
    }
  };

  if (state === "calendar_processing") {
    const label = calendarProcessingTitle
      ? `Scheduling ${calendarProcessingTitle.slice(0, 40)}…`
      : "Scheduling…";
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className={`scard compact cworking ${isVisible ? "" : "leaving"}`}>
          {sayThisRow(label.replace("...", ""))}
        </div>
      </div>
    );
  }

  if (state === "calendar_success" && calendarSuccess) {
    // Extremely clean deterministic success — same bg/rounded as cancel toast (scancel)
    // Left two words "Calendar booked", right green Badge, pill dynamically extends with horizontal padding
    // Not bloated with title/meta — AI success_message is 2-5 words and already validated, but we keep UI deterministic
    const driftY = position === "top" ? -10 : 10;
    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <motion.div
          className="scard scancel"
          style={{
            width: "fit-content",
            minWidth: 180,
            maxWidth: 380,
            paddingLeft: 16,
            paddingRight: 12,
            gap: 12,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            borderRadius: 9999,
            background: "var(--color-background)",
          }}
          initial={dialogEnter(driftY)}
          animate={{ ...dialogShown, transition: dialogTransition }}
        >
          <span
            className="scancel-label"
            style={{ whiteSpace: "nowrap", fontWeight: 500 }}
          >
            {t("overlay.calendarBooked")}
          </span>
          <div className="scancel-actions">
            <Badge
              variant="green"
              className="rounded-full text-[12px] px-2.5 py-1"
            >
              {t("overlay.booked")}
            </Badge>
          </div>
        </motion.div>
      </div>
    );
  }

  if (state === "calendar_clarify" && calendarClarify) {
    const handleClarifySubmit = async () => {
      const answer = clarifyInput.trim();
      if (!answer) return;
      const original = pendingCalendarTranscript || "";
      const combined = original ? `${original} - Answer: ${answer}` : answer;
      // Clear input immediately for clean UX
      setClarifyInput("");
      // Keep pill visible while we re-process — switch to say_this spinner
      setState("say_this");
      setSayThisLabel("Scheduling");
      try {
        // Use the generated command via Tauri invoke (fallback to direct if not in bindings)
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("submit_calendar_clarification", { transcript: combined });
      } catch (err) {
        console.error("Failed to submit calendar clarification:", err);
        // Fallback: show failure locally
        setState("calendar_failure");
        setCalendarFailure("Couldn't update calendar.");
      }
    };

    const driftY = position === "top" ? -10 : 10;
    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <motion.div
          className="scard"
          style={{
            width: "fit-content",
            minWidth: 320,
            maxWidth: 480,
            background: "#f5f5f4", // stone-100
            border: "none",
            borderRadius: 24, // rounded large
            padding: "16px 20px", // padding Y extended, X slightly extended
            boxShadow:
              "0 4px 24px rgba(0,0,0,0.08), 0 1px 4px rgba(0,0,0,0.06)",
            display: "flex",
            flexDirection: "column",
            gap: 12,
          }}
          initial={dialogEnter(driftY)}
          animate={{ ...dialogShown, transition: dialogTransition }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Badge
              variant="orange"
              className="rounded-full text-[11px] px-2.5 py-1"
            >
              ?
            </Badge>
            <span
              style={{
                fontSize: 13,
                fontWeight: 500,
                color: "#44403c",
                lineHeight: 1.3,
              }}
            >
              {calendarClarify}
            </span>
          </div>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              background: "white",
              borderRadius: 9999,
              padding: "8px 8px 8px 16px",
              border: "1px solid #e7e5e4",
              boxShadow: "0 1px 2px rgba(0,0,0,0.04)",
            }}
          >
            <input
              value={clarifyInput}
              onChange={(e) => setClarifyInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void handleClarifySubmit();
                }
                if (e.key === "Escape") {
                  void commands.hideResultOverlay();
                }
              }}
              placeholder="Type date/time..."
              autoFocus
              style={{
                flex: 1,
                border: "none",
                outline: "none",
                background: "transparent",
                fontSize: 14,
                fontWeight: 400,
                color: "#1c1917",
                minWidth: 0,
              }}
            />
            <button
              onClick={() => void handleClarifySubmit()}
              disabled={!clarifyInput.trim()}
              aria-label="Send"
              style={{
                width: 32,
                height: 32,
                borderRadius: 9999,
                background: clarifyInput.trim() ? "#1c1917" : "#e7e5e4",
                color: clarifyInput.trim() ? "white" : "#a8a29e",
                border: "none",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                cursor: clarifyInput.trim() ? "pointer" : "not-allowed",
                transition: "all 0.15s ease",
                flexShrink: 0,
              }}
            >
              <ArrowUp size={16} weight="bold" aria-hidden="true" />
            </button>
          </div>
        </motion.div>
      </div>
    );
  }

  if (state === "calendar_failure" && calendarFailure) {
    return (
      <div
        dir={direction}
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className={`scard compact cworking ${isVisible ? "" : "leaving"}`}>
          <div className="sbase">
            <div className="sbase-l">
              <IOSSpinner size={13} color="#dc2626" speed={1.0} />
            </div>
            <span className="swork-label" style={{ color: "#dc2626" }}>
              {calendarFailure}
            </span>
            <div className="sbase-r">
              <Badge variant="rose" className="rounded-[7px] text-[11px]">
                !
              </Badge>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // ---- Minimal overlay: exactly one row at a time — waveform (recording), or a
  // spinner + label (transcribing / processing). Never both. The pill animates its
  // width between them; the cancel button is in both rows so it stays put.
  // "editing" remains for edit mode only. Standard transcribing/processing now
  // uses random LOADING_STATES (not "Transcribing...") — single spinner left, extremely clean.
  const working =
    state === "transcribing" ||
    state === "processing" ||
    state === "prompting" ||
    state === "editing";
  const workLabel =
    state === "editing"
      ? t("overlay.editing", { defaultValue: "Editing" })
      : state === "prompting"
        ? t("overlay.prompting", { defaultValue: "Prompting" })
        : state === "transcribing" || state === "processing"
          ? `${standardLoadingLabel}...`
          : t("overlay.transcribing");

  return (
    <div
      dir={direction}
      className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
    >
      <div
        className={`scard compact ${working && isVisible ? "cworking" : ""} ${state === "prompting" || state === "editing" ? "ai-prompting" : ""}`}
      >
        {working ? workingRow(workLabel, true) : listeningRow(true, false)}
      </div>
    </div>
  );
};

export default RecordingOverlay;
