import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { platform } from "@tauri-apps/plugin-os";
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission,
  checkMicrophonePermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import SuperFlowTextLogo from "../icons/SuperFlowTextLogo";
import {
  Keyboard,
  Microphone,
  Check,
  CircleNotch,
} from "@phosphor-icons/react";
import { Badge } from "../ui/Badge";

interface AccessibilityOnboardingProps {
  onComplete: () => void;
}

type PermissionStatus = "checking" | "needed" | "waiting" | "granted";
type PermissionPlatform = "macos" | "windows" | "other";

interface PermissionsState {
  accessibility: PermissionStatus;
  microphone: PermissionStatus;
}

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const [permissionPlatform, setPermissionPlatform] =
    useState<PermissionPlatform | null>(null);
  const [permissions, setPermissions] = useState<PermissionsState>({
    accessibility: "checking",
    microphone: "checking",
  });
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef<number>(0);
  const MAX_POLLING_ERRORS = 3;

  const isMacOS = permissionPlatform === "macos";
  const isWindows = permissionPlatform === "windows";
  const showMicrophonePermission = isMacOS || isWindows;
  const showAccessibilityPermission = isMacOS;

  const allGranted = isMacOS
    ? permissions.accessibility === "granted" &&
      permissions.microphone === "granted"
    : isWindows
      ? permissions.microphone === "granted"
      : true;

  const completeOnboarding = useCallback(async () => {
    await Promise.all([refreshAudioDevices(), refreshOutputDevices()]);
    timeoutRef.current = setTimeout(() => onComplete(), 300);
  }, [onComplete, refreshAudioDevices, refreshOutputDevices]);

  const hasWindowsMicrophoneAccess = useCallback(async (): Promise<boolean> => {
    const microphoneStatus =
      await commands.getWindowsMicrophonePermissionStatus();

    if (!microphoneStatus.supported) {
      return true;
    }

    return microphoneStatus.overall_access !== "denied";
  }, []);

  // Check platform and permission status on mount
  useEffect(() => {
    const currentPlatform = platform();
    const nextPlatform: PermissionPlatform =
      currentPlatform === "macos"
        ? "macos"
        : currentPlatform === "windows"
          ? "windows"
          : "other";

    setPermissionPlatform(nextPlatform);

    // Skip immediately on unsupported platforms
    if (nextPlatform === "other") {
      onComplete();
      return;
    }

    const checkInitial = async () => {
      if (nextPlatform === "macos") {
        try {
          const [accessibilityGranted, microphoneGranted] = await Promise.all([
            checkAccessibilityPermission(),
            checkMicrophonePermission(),
          ]);

          // If accessibility is granted, initialize Enigo and shortcuts
          if (accessibilityGranted) {
            try {
              await Promise.all([
                commands.initializeEnigo(),
                commands.initializeShortcuts(),
              ]);
            } catch (e) {
              console.warn("Failed to initialize after permission grant:", e);
            }
          }

          const newState: PermissionsState = {
            accessibility: accessibilityGranted ? "granted" : "needed",
            microphone: microphoneGranted ? "granted" : "needed",
          };

          setPermissions(newState);

          if (accessibilityGranted && microphoneGranted) {
            await completeOnboarding();
          }
        } catch (error) {
          console.error("Failed to check macOS permissions:", error);
          toast.error(t("onboarding.permissions.errors.checkFailed"));
          setPermissions({
            accessibility: "needed",
            microphone: "needed",
          });
        }

        return;
      }

      try {
        const microphoneGranted = await hasWindowsMicrophoneAccess();

        setPermissions({
          accessibility: "granted",
          microphone: microphoneGranted ? "granted" : "needed",
        });

        if (microphoneGranted) {
          await completeOnboarding();
        }
      } catch (error) {
        console.warn("Failed to check Windows microphone permissions:", error);
        setPermissions({
          accessibility: "granted",
          microphone: "granted",
        });
        await completeOnboarding();
      }
    };

    checkInitial();
  }, [completeOnboarding, hasWindowsMicrophoneAccess, onComplete, t]);

  // Polling for permissions after user clicks a button
  const startPolling = useCallback(() => {
    if (pollingRef.current || permissionPlatform === null) return;

    pollingRef.current = setInterval(async () => {
      try {
        if (permissionPlatform === "windows") {
          const microphoneGranted = await hasWindowsMicrophoneAccess();

          if (microphoneGranted) {
            setPermissions((prev) => ({ ...prev, microphone: "granted" }));

            if (pollingRef.current) {
              clearInterval(pollingRef.current);
              pollingRef.current = null;
            }

            await completeOnboarding();
          }

          errorCountRef.current = 0;
          return;
        }

        const [accessibilityGranted, microphoneGranted] = await Promise.all([
          checkAccessibilityPermission(),
          checkMicrophonePermission(),
        ]);

        setPermissions((prev) => {
          const newState = { ...prev };

          if (accessibilityGranted && prev.accessibility !== "granted") {
            newState.accessibility = "granted";
            // Initialize Enigo and shortcuts when accessibility is granted
            Promise.all([
              commands.initializeEnigo(),
              commands.initializeShortcuts(),
            ]).catch((e) => {
              console.warn("Failed to initialize after permission grant:", e);
            });
          }

          if (microphoneGranted && prev.microphone !== "granted") {
            newState.microphone = "granted";
          }

          return newState;
        });

        // If both granted, stop polling, refresh audio devices, and proceed
        if (accessibilityGranted && microphoneGranted) {
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }
          await completeOnboarding();
        }

        // Reset error count on success
        errorCountRef.current = 0;
      } catch (error) {
        console.error("Error checking permissions:", error);
        errorCountRef.current += 1;

        if (errorCountRef.current >= MAX_POLLING_ERRORS) {
          // Stop polling after too many consecutive errors
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }
          toast.error(t("onboarding.permissions.errors.checkFailed"));
        }
      }
    }, 1000);
  }, [completeOnboarding, hasWindowsMicrophoneAccess, permissionPlatform, t]);

  // Cleanup polling and timeouts on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
      }
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleGrantAccessibility = async () => {
    try {
      await requestAccessibilityPermission();
      setPermissions((prev) => ({ ...prev, accessibility: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request accessibility permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const handleGrantMicrophone = async () => {
    try {
      if (isWindows) {
        await commands.openMicrophonePrivacySettings();
      } else {
        await requestMicrophonePermission();
      }

      setPermissions((prev) => ({ ...prev, microphone: "waiting" }));
      startPolling();
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const isChecking =
    permissionPlatform === null ||
    (isMacOS &&
      permissions.accessibility === "checking" &&
      permissions.microphone === "checking") ||
    (isWindows && permissions.microphone === "checking");

  // Still checking platform/initial permissions
  if (isChecking) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-neutral-900">
        <CircleNotch className="size-8 animate-spin text-text/50" />
      </div>
    );
  }

  // All permissions granted - show success briefly
  if (allGranted) {
    return (
      <div className="flex h-screen w-screen flex-col items-center justify-center gap-4 bg-neutral-900">
        <div className="rounded-full bg-emerald-500/20 p-4">
          <Check className="size-12 text-emerald-400" />
        </div>
        <p className="text-lg font-medium text-text">
          {t("onboarding.permissions.allGranted")}
        </p>
      </div>
    );
  }

  // Show permissions request screen
  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-6 bg-neutral-900 p-6">
      <div className="flex flex-col items-center gap-2">
        <SuperFlowTextLogo size={22} />
      </div>

      <div className="flex w-full max-w-md flex-col items-center gap-4">
        <div className="mb-2 text-center">
          <h2 className="mb-2 text-xl font-semibold text-text">
            {t("onboarding.permissions.title")}
          </h2>
          <p className="text-text/70">
            {t("onboarding.permissions.description")}
          </p>
        </div>

        {/* Microphone Permission Card */}
        {showMicrophonePermission && (
          <div className="w-full rounded-lg bg-surface p-4">
            <div className="flex items-start gap-4">
              <Microphone size={32} className="shrink-0 text-stone-100" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-medium tracking-tight text-stone-50">
                    {t("onboarding.permissions.microphone.title")}
                  </h3>
                  {permissions.microphone === "granted" && (
                    <Badge variant="green">
                      {t("onboarding.permissions.granted")}
                    </Badge>
                  )}
                  {permissions.microphone === "waiting" && (
                    <Badge variant="neutral">
                      <CircleNotch className="size-3 animate-spin" />
                      {t("onboarding.permissions.waiting")}
                    </Badge>
                  )}
                </div>
                <p className="mt-1 text-sm leading-relaxed text-stone-400">
                  {t("onboarding.permissions.microphone.description")}
                </p>
                {permissions.microphone !== "granted" &&
                  permissions.microphone !== "waiting" && (
                    <div className="mt-3">
                      <PermissionButton onClick={handleGrantMicrophone}>
                        {isWindows
                          ? t("accessibility.openSettings")
                          : t("onboarding.permissions.grant")}
                      </PermissionButton>
                    </div>
                  )}
              </div>
            </div>
          </div>
        )}

        {/* Accessibility Permission Card */}
        {showAccessibilityPermission && (
          <div className="w-full rounded-lg bg-surface p-4">
            <div className="flex items-start gap-4">
              <Keyboard size={32} className="shrink-0 text-stone-100" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-medium tracking-tight text-stone-50">
                    {t("onboarding.permissions.accessibility.title")}
                  </h3>
                  {permissions.accessibility === "granted" && (
                    <Badge variant="green">
                      {t("onboarding.permissions.granted")}
                    </Badge>
                  )}
                  {permissions.accessibility === "waiting" && (
                    <Badge variant="neutral">
                      <CircleNotch className="size-3 animate-spin" />
                      {t("onboarding.permissions.waiting")}
                    </Badge>
                  )}
                </div>
                <p className="mt-1 text-sm leading-relaxed text-stone-400">
                  {t("onboarding.permissions.accessibility.description")}
                </p>
                {permissions.accessibility !== "granted" &&
                  permissions.accessibility !== "waiting" && (
                    <div className="mt-3">
                      <PermissionButton onClick={handleGrantAccessibility}>
                        {t("onboarding.permissions.grant")}
                      </PermissionButton>
                    </div>
                  )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

/* Grant button - fixed geometry, stone-950 surface with blue edge and the
   shared inset/ambient shadow stack. */
const PermissionButton = ({
  onClick,
  children,
}: {
  onClick: () => void;
  children: React.ReactNode;
}) => {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group relative inline-flex h-7 cursor-pointer items-center justify-center whitespace-nowrap rounded-[5px] border border-blue-700 bg-blue-600 px-3.5 py-1 no-underline shadow-[0_0_0_1px_#2563eb26,inset_0_2px_#ffffff30,inset_0_-0.5px_2px_#00000065,0_2px_8px_#0000000d,0_3px_4px_#00000040] transition-[background,border-color,box-shadow] duration-200 ease-out hover:border-blue-700 hover:bg-blue-700/[0.85] hover:shadow-[0_0_0_1px_#1d4ed833,inset_0_2px_#ffffff22,inset_0_-0.5px_2px_#00000080,0_2px_8px_#00000012,0_3px_4px_#0000004d]"
    >
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 rounded-[5px] bg-blue-950/15 opacity-0 transition-opacity duration-200 ease-out group-hover:opacity-100"
      />
      <span className="relative z-10 inline-flex items-center justify-center gap-1.5">
        <span className="text-[13px] font-[460] tracking-[0.15px] text-white">
          {children}
        </span>
      </span>
    </button>
  );
};

export default AccessibilityOnboarding;
