import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy } from "@phosphor-icons/react";
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

type OverlayState =
  | "recording"
  | "streaming"
  | "transcribing"
  | "processing"
  | "prompting"
  | "ai_notice";

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

// Number of reactive bars in the waveform (the simple, smoothed style shared by
// every overlay form). Mic levels arrive as 16 FFT buckets; we take the first N.
const WAVE_BARS = 9;

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
  const [levels, setLevels] = useState<number[]>(Array(WAVE_BARS).fill(0));
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
        // Reset synchronously before settings I/O. A fast microphone can emit
        // recording-ready while the awaits below are in flight; resetting after
        // them would overwrite that event and leave the overlay stuck arming.
        if (overlayState === "recording" || overlayState === "streaming") {
          setCaptureReady(false);
          smoothedLevelsRef.current = Array(16).fill(0);
          setLevels(Array(WAVE_BARS).fill(0));
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
        // Exponential smoothing across the 16 buckets, then take the first N
        // bars for the shared waveform.
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, WAVE_BARS));
      });

      const unlistenStream = await events.streamTextEvent.listen((event) => {
        setStreamText(event.payload);
      });

      const unlistenPhase = await events.streamPhaseEvent.listen((event) => {
        const payload: StreamPhaseEvent = event.payload;
        setPhase(payload.phase);
        if (payload.kind) setWorkKind(payload.kind);
      });

      return () => {
        unlistenShow();
        unlistenHide();
        unlistenReady();
        unlistenLevel();
        unlistenStream();
        unlistenPhase();
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
  useEffect(() => {
    if (state !== "streaming" || !isVisible || !captureReady) return;
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
              className="group relative inline-flex h-7 cursor-pointer items-center justify-center whitespace-nowrap rounded-[9px] border border-stone-700/70 bg-[linear-gradient(#1816131a_0%_100%),linear-gradient(#403c38_0%,#1c1917_100%)] px-3.5 py-1 no-underline shadow-[0_0_0_1px_#29252426,inset_0_2px_#ffffff24,inset_0_-0.5px_2px_#00000070,0_2px_8px_#0000000a,0_3px_4px_#00000036] transition-[background,border-color,box-shadow] duration-200 ease-out hover:border-stone-800 hover:bg-[linear-gradient(#12100e1a_0%_100%),linear-gradient(#312d29_0%,#141210_100%)] hover:shadow-[0_0_0_1px_#1c191733,inset_0_2px_#ffffff18,inset_0_-0.5px_2px_#00000085,0_2px_8px_#00000010,0_3px_4px_#00000042] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-stone-600"
              whileTap={{ scale: 0.985 }}
              transition={{
                duration: 0.1,
                ease: [0.22, 1, 0.36, 1],
              }}
            >
              <span
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 rounded-[9px] bg-stone-950/25 opacity-0 transition-opacity duration-200 ease-out group-hover:opacity-100"
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
                      className="inline-flex items-center justify-center gap-1.5 text-[14px] font-[460] tracking-[0.15px] text-stone-50"
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
                      className="inline-flex items-center justify-center gap-1.5 text-[13px] font-[460] tracking-[0.15px] text-stone-50"
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
  const waveform = (
    <div className={`swave ${captureReady ? "ready" : "arming"}`}>
      {levels.map((v, i) => (
        <i
          key={i}
          style={{
            height: `${Math.max(3, Math.min(18, 3 + Math.pow(v, 0.7) * 15))}px`,
          }}
        />
      ))}
    </div>
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
        <span className="sspinner" />
      </div>
      <span className="swork-label">{label}</span>
      <div className="sbase-r">{showCancel && cancelBtn}</div>
    </div>
  );

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
                workKind === "polishing"
                  ? t("overlay.processing")
                  : t("overlay.transcribing"),
                true,
              )
            : listeningRow(open, true)}
        </div>
      </div>
    );
  }

  // ---- Minimal overlay: exactly one row at a time — waveform (recording), or a
  // spinner + label (transcribing / processing). Never both. The pill animates its
  // width between them; the cancel button is in both rows so it stays put.
  const working =
    state === "transcribing" || state === "processing" || state === "prompting";
  const workLabel =
    state === "prompting"
      ? t("overlay.prompting", { defaultValue: "Prompting" })
      : state === "processing"
        ? t("overlay.processing")
        : t("overlay.transcribing");

  return (
    <div
      dir={direction}
      className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
    >
      <div
        className={`scard compact ${working && isVisible ? "cworking" : ""} ${state === "prompting" ? "ai-prompting" : ""}`}
      >
        {working ? workingRow(workLabel, true) : listeningRow(false, true)}
      </div>
    </div>
  );
};

export default RecordingOverlay;
