import { useEffect, useState, useRef, type ReactNode } from "react";
import { toast, Toaster } from "sonner";
import { CheckCircle, Warning, XCircle } from "@phosphor-icons/react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { ModelStateEvent, RecordingErrorEvent } from "./lib/types/events";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import SecureInputWarning from "./components/SecureInputWarning";
import { CleanupModelToast } from "./components/CleanupModelToast";
import Footer from "./components/footer";
import Onboarding, { AccessibilityOnboarding } from "./components/onboarding";
import { Introduction } from "./components/introduction";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import SidebarToggleIcon from "./components/icons/SidebarToggleIcon";
import { WhatsNewGate } from "./components/whats-new";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { commands } from "@/bindings";
import { getLanguageDirection, initializeRTL } from "@/lib/utils/rtl";

type OnboardingStep = "introduction" | "accessibility" | "model" | "done";

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const { t, i18n } = useTranslation();
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );
  // Track if this is a returning user who just needs to grant permissions
  // (vs a new user who needs full onboarding including model selection)
  const [isReturningUser, setIsReturningUser] = useState(false);
  const [currentSection, setCurrentSection] = useState<SidebarSection>("home");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [windowFullPage, setWindowFullPage] = useState(false);
  const { settings, updateSetting } = useSettings();
  const direction = getLanguageDirection(i18n.language);
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // Initialize RTL direction when language changes
  useEffect(() => {
    initializeRTL(i18n.language);
  }, [i18n.language]);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let active = true;
    let resizeTimer: ReturnType<typeof setTimeout> | undefined;

    const syncWindowState = async () => {
      const [maximized, fullscreen] = await Promise.all([
        appWindow.isMaximized(),
        appWindow.isFullscreen(),
      ]);
      if (active) setWindowFullPage(maximized || fullscreen);
    };

    const unlisten = appWindow.onResized(() => {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => {
        syncWindowState().catch((error) => {
          console.warn("Failed to read window state:", error);
        });
      }, 80);
    });

    syncWindowState().catch((error) => {
      console.warn("Failed to read window state:", error);
    });

    return () => {
      active = false;
      clearTimeout(resizeTimer);
      unlisten.then((stopListening) => stopListening());
    };
  }, []);

  // Initialize Enigo, shortcuts, and refresh audio devices when main app loads
  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => {
        console.warn("Failed to initialize:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Ctrl+B (Windows/Linux) or Cmd+B (macOS) toggles the sidebar
      if (
        event.key.toLowerCase() === "b" &&
        (event.ctrlKey || event.metaKey) &&
        !event.shiftKey
      ) {
        event.preventDefault();
        setSidebarOpen((open) => !open);
        return;
      }

      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  // Listen for recording errors from the backend and show a toast
  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "cleanup_model_not_ready") {
        toast.error(t("errors.cleanupModelNotReadyTitle"), {
          description: t("errors.cleanupModelNotReady"),
        });
      } else if (error_type === "microphone_permission_denied") {
        const currentPlatform = platform();
        const platformKey = `errors.micPermissionDenied.${currentPlatform}`;
        const description = t(platformKey, {
          defaultValue: t("errors.micPermissionDenied.generic"),
        });
        toast.error(t("errors.micPermissionDeniedTitle"), { description });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else {
        toast.error(
          t("errors.recordingFailed", { error: detail ?? "Unknown error" }),
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for paste failures and show a toast.
  // The technical error detail is logged to superflow.log on the Rust side
  // (see actions.rs `error!("Failed to paste transcription: ...")`),
  // so we show a localized, user-friendly message here instead of the raw error.
  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for transcription failures and show a toast.
  // The payload is the backend error message (also logged to superflow.log).
  useEffect(() => {
    const unlisten = listen<string>("transcription-error", (event) => {
      toast.error(t("errors.transcriptionFailedTitle"), {
        description: event.payload,
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for model loading failures and show a toast
  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: event.payload.error,
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  const revealMainWindowForPermissions = async () => {
    try {
      await commands.showMainWindowCommand();
    } catch (e) {
      console.warn("Failed to show main window for permission onboarding:", e);
    }
  };

  const checkOnboardingStatus = async () => {
    try {
      const settingsResult = await commands.getAppSettings();
      const hasCompletedOnboarding =
        settingsResult.status === "ok" &&
        settingsResult.data.onboarding_completed === true;
      const currentPlatform = platform();

      if (hasCompletedOnboarding) {
        // Returning user - check if they need to grant permissions first
        setIsReturningUser(true);

        if (currentPlatform === "macos") {
          try {
            const [hasAccessibility, hasMicrophone] = await Promise.all([
              checkAccessibilityPermission(),
              checkMicrophonePermission(),
            ]);
            if (!hasAccessibility || !hasMicrophone) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check macOS permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        if (currentPlatform === "windows") {
          try {
            const microphoneStatus =
              await commands.getWindowsMicrophonePermissionStatus();
            if (
              microphoneStatus.supported &&
              microphoneStatus.overall_access === "denied"
            ) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check Windows microphone permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        setOnboardingStep("done");
      } else {
        // New user - introduction first, then permissions, then model choice
        setIsReturningUser(false);
        setOnboardingStep("introduction");
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("introduction");
    }
  };

  const handleIntroductionComplete = () => {
    setOnboardingStep("accessibility");
  };

  const handleAccessibilityComplete = () => {
    // Returning users already have models, skip to main app
    // New users need to select a model
    setOnboardingStep(isReturningUser ? "done" : "model");
  };

  const handleModelSelected = () => {
    // The S1-mini clean-up model is optional and disabled by default; it can
    // be enabled later from the model card. Onboarding ends at model choice.
    setOnboardingStep("done");
  };

  // Rendered once around every step below (including onboarding) so
  // toast.error() calls surface to the user. sonner renders via a portal, so
  // its position in the tree doesn't affect layout. Without this, errors during
  // onboarding (e.g. a model download failing because blob.handy.computer is
  // unreachable) are silently swallowed and the wizard just appears to "blink".
  const toaster = (
    <Toaster
      theme="system"
      position="bottom-right"
      icons={{
        success: (
          <CheckCircle weight="fill" className="size-4 text-[#34D399]" />
        ),
        error: <XCircle weight="fill" className="size-4 text-[#FF5C5C]" />,
        warning: <Warning weight="fill" className="size-4 text-[#FF6A1A]" />,
      }}
      toastOptions={{
        unstyled: true,
        classNames: {
          toast:
            "bg-surface text-text border-0 rounded-[7px] shadow-none px-3 py-3 flex items-center gap-2 text-sm",
          error: "!bg-[#241010] !text-[#FFD3D3]",
          warning: "!bg-[#241708] !text-[#FFDCC0]",
          success: "!bg-[#0D1E16] !text-[#C8F5DA]",
          title: "font-normal text-current",
          description: "text-current opacity-75",
          actionButton:
            "cursor-pointer whitespace-nowrap rounded-lg border border-current/20 bg-transparent px-2 py-1 text-xs font-medium text-current hover:bg-current/10 hover:text-current focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-current",
          cancelButton:
            "cursor-pointer whitespace-nowrap rounded-lg border-0 bg-current/10 px-2 py-1 text-xs font-medium text-current hover:bg-current/15 hover:text-current focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-current",
          closeButton:
            "border-0 bg-transparent text-current shadow-none hover:bg-current/10 hover:text-current focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-current",
        },
      }}
    />
  );

  // Still checking onboarding status
  if (onboardingStep === null) {
    return null;
  }

  // Select the content for the current step. The Toaster is rendered once, in a
  // stable wrapper around this node, so crossing between onboarding steps and
  // the main app never remounts it (which would drop any in-flight toast).
  let content: ReactNode;
  if (onboardingStep === "introduction") {
    content = <Introduction onComplete={handleIntroductionComplete} />;
  } else if (onboardingStep === "accessibility") {
    content = (
      <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />
    );
  } else if (onboardingStep === "model") {
    content = <Onboarding onModelSelected={handleModelSelected} />;
  } else {
    content = (
      <div
        dir={direction}
        className="h-screen flex flex-col select-none cursor-default bg-transparent"
      >
        <ErrorBoundary context="What's New">
          <WhatsNewGate />
        </ErrorBoundary>
        {/* Main content area that takes remaining space */}
        <div className="flex-1 flex overflow-hidden">
          <Sidebar
            activeSection={currentSection}
            onSectionChange={setCurrentSection}
            open={sidebarOpen}
            onToggle={() => setSidebarOpen((open) => !open)}
            opaque={windowFullPage}
          />
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-background">
            <div
              dir="ltr"
              data-tauri-drag-region
              className="app-titlebar flex h-11 shrink-0 items-center px-2"
            >
              {!sidebarOpen && (
                <button
                  type="button"
                  onClick={() => setSidebarOpen(true)}
                  aria-label="Open sidebar"
                  title="Open sidebar (Ctrl+B)"
                  className="flex size-7 items-center justify-center rounded-md text-stone-500 transition-colors duration-150 hover:bg-white/[0.06] hover:text-stone-200"
                >
                  <SidebarToggleIcon expanded={false} />
                </button>
              )}
            </div>
            <div className="flex-1 overflow-y-auto">
              <div className="flex flex-col items-center p-4 gap-4">
                <AccessibilityPermissions />
                <SecureInputWarning />
                {renderSettingsContent(currentSection)}
              </div>
            </div>
            <Footer />
          </div>
        </div>
      </div>
    );
  }

  return (
    <>
      {toaster}
      {/* Global live indicator for the background clean-up model install. */}
      <CleanupModelToast />
      <div
        className={
          onboardingStep === "done"
            ? "h-screen w-screen bg-transparent"
            : "h-screen w-screen bg-background"
        }
      >
        {content}
      </div>
    </>
  );
}

export default App;
