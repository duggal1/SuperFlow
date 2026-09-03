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
import { HugeiconsIcon } from "@hugeicons/react";
import {
  KeyboardIcon,
  Mic02Icon,
  CheckmarkCircle02Icon,
} from "@hugeicons/core-free-icons";
import { IOSSpinner } from "../shared/global-spinner";

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
      <div className="flex h-screen w-screen items-center justify-center sidebar-material">
        <IOSSpinner size={20} color="var(--color-text)" />
      </div>
    );
  }

  // All permissions granted - show success briefly
  if (allGranted) {
    return (
      <div className="flex h-screen w-screen flex-col items-center justify-center gap-4 sidebar-material">
        <HugeiconsIcon
          icon={CheckmarkCircle02Icon}
          size={48}
          className="text-green-600 dark:text-green-500"
        />
        <p className="text-lg font-medium text-text">
          {t("onboarding.permissions.allGranted")}
        </p>
      </div>
    );
  }

  // Show permissions request screen
  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-6 sidebar-material p-6">
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
          <div className="w-full rounded-lg border border-stone-200/70 bg-white dark:border-white/[0.06] dark:bg-white/[0.04] p-4">
            <div className="flex items-start gap-4">
              <HugeiconsIcon
                icon={Mic02Icon}
                size={28}
                className="shrink-0 text-stone-700 dark:text-stone-100"
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-medium tracking-tight text-stone-900 dark:text-stone-50">
                    {t("onboarding.permissions.microphone.title")}
                  </h3>
                  {permissions.microphone === "granted" && (
                    <span className="inline-flex items-center gap-1.5 text-sm">
                      <HugeiconsIcon
                        icon={CheckmarkCircle02Icon}
                        size={16}
                        className="text-green-600 dark:text-green-500"
                      />
                      <span className="text-green-600 dark:text-green-500 font-medium">
                        {t("onboarding.permissions.granted")}
                      </span>
                    </span>
                  )}
                  {permissions.microphone === "waiting" && (
                    <span className="inline-flex items-center gap-1.5 text-sm text-stone-500 dark:text-stone-400">
                      <IOSSpinner size={14} color="currentColor" speed={1.2} />
                      {t("onboarding.permissions.waiting")}
                    </span>
                  )}
                </div>
                <p className="mt-1 text-sm leading-relaxed text-stone-600 dark:text-stone-400">
                  {t("onboarding.permissions.microphone.description")}
                </p>
                {permissions.microphone !== "granted" &&
                  permissions.microphone !== "waiting" && (
                    <div className="mt-3">
                      <PermissionButton onClick={handleGrantMicrophone} isLoading={false}>
                        {isWindows
                          ? t("accessibility.openSettings")
                          : t("onboarding.permissions.grant")}
                      </PermissionButton>
                    </div>
                  )}
                {permissions.microphone === "waiting" && (
                  <div className="mt-3">
                    <PermissionButton onClick={handleGrantMicrophone} isLoading={true}>
                      {t("onboarding.permissions.waiting")}
                    </PermissionButton>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Accessibility Permission Card */}
        {showAccessibilityPermission && (
          <div className="w-full rounded-lg border border-stone-200/70 bg-white dark:border-white/[0.06] dark:bg-white/[0.04] p-4">
            <div className="flex items-start gap-4">
              <HugeiconsIcon
                icon={KeyboardIcon}
                size={28}
                className="shrink-0 text-stone-700 dark:text-stone-100"
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-medium tracking-tight text-stone-900 dark:text-stone-50">
                    {t("onboarding.permissions.accessibility.title")}
                  </h3>
                  {permissions.accessibility === "granted" && (
                    <span className="inline-flex items-center gap-1.5 text-sm">
                      <HugeiconsIcon
                        icon={CheckmarkCircle02Icon}
                        size={16}
                        className="text-green-600 dark:text-green-500"
                      />
                      <span className="text-green-600 dark:text-green-500 font-medium">
                        {t("onboarding.permissions.granted")}
                      </span>
                    </span>
                  )}
                  {permissions.accessibility === "waiting" && (
                    <span className="inline-flex items-center gap-1.5 text-sm text-stone-500 dark:text-stone-400">
                      <IOSSpinner size={14} color="currentColor" speed={1.2} />
                      {t("onboarding.permissions.waiting")}
                    </span>
                  )}
                </div>
                <p className="mt-1 text-sm leading-relaxed text-stone-600 dark:text-stone-400">
                  {t("onboarding.permissions.accessibility.description")}
                </p>
                {permissions.accessibility !== "granted" &&
                  permissions.accessibility !== "waiting" && (
                    <div className="mt-3">
                      <PermissionButton onClick={handleGrantAccessibility} isLoading={false}>
                        {t("onboarding.permissions.grant")}
                      </PermissionButton>
                    </div>
                  )}
                {permissions.accessibility === "waiting" && (
                  <div className="mt-3">
                    <PermissionButton onClick={handleGrantAccessibility} isLoading={true}>
                      {t("onboarding.permissions.waiting")}
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

/* Grant button - blue theme, clean, with optional iOS spinner prefix */
const PermissionButton = ({
  onClick,
  children,
  isLoading = false,
}: {
  onClick: () => void;
  children: React.ReactNode;
  isLoading?: boolean;
}) => {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={isLoading}
      className="group relative inline-flex h-7 cursor-pointer items-center justify-center whitespace-nowrap rounded-[5px] border border-blue-700 bg-blue-600 px-3.5 py-1 no-underline shadow-[0_0_0_1px_#2563eb26,inset_0_2px_#ffffff30,inset_0_-0.5px_2px_#00000065,0_2px_8px_#0000000d,0_3px_4px_#00000040] transition-[background,border-color,box-shadow] duration-200 ease-out hover:border-blue-700 hover:bg-blue-700/[0.85] hover:shadow-[0_0_0_1px_#1d4ed833,inset_0_2px_#ffffff22,inset_0_-0.5px_2px_#00000080,0_2px_8px_#00000012,0_3px_4px_#0000004d] disabled:cursor-not-allowed disabled:opacity-70"
    >
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 rounded-[5px] bg-blue-950/15 opacity-0 transition-opacity duration-200 ease-out group-hover:opacity-100"
      />
      <span className="relative z-10 inline-flex items-center justify-center gap-1.5">
        {isLoading && <IOSSpinner size={12} color="white" speed={1.0} />}
        <span className="text-[13px] font-[460] tracking-[0.15px] text-white">
          {children}
        </span>
      </span>
    </button>
  );
};

export default AccessibilityOnboarding;
