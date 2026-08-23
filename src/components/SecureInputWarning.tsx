import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut, Warning, X } from "@phosphor-icons/react";
import { commands, type SecureInputStatus } from "@/bindings";

// Detailed remediation steps live in the docs rather than in the banner
export const SECURE_INPUT_HELP_URL =
  "https://superflow.computer/docs/troubleshooting#shortcuts-stopped-working-on-macos-secure-input";

/**
 * Compact warning banner shown while macOS Secure Input is stuck on.
 *
 * Secure Input (password fields, Terminal's "Secure Keyboard Entry", a stuck
 * loginwindow) blocks key events from reaching SuperFlow's keyboard listener, so
 * keyed shortcuts silently stop firing (issue #1578). The backend monitor
 * emits `secure-input-changed` on state transitions; `sustained` filters out
 * the normal momentary activation from focusing a password field.
 */
const SecureInputWarning: React.FC = () => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<SecureInputStatus | null>(null);
  const [dismissed, setDismissed] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await commands.getSecureInputStatus());
    } catch (e) {
      console.warn("Failed to fetch secure input status:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = listen<SecureInputStatus>(
      "secure-input-changed",
      (event) => setStatus(event.payload),
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // Only warn when the user is actually impacted: a binding is degraded
  // (side-specific matching widened) or dead (e.g. fn+key), or they ran into
  // the blocked shortcut recorder. When the fallback covers everything
  // transparently — and nothing else surfaced — stay silent; the backend
  // still logs.
  const impacted =
    status !== null &&
    ((status.sustained &&
      (status.degraded_bindings.length > 0 ||
        status.uncovered_bindings.length > 0)) ||
      status.recorder_blocked);

  // A dismissal lasts for the current episode only: once the condition
  // clears, the next occurrence warns again. The tray badge is the
  // persistent indicator and is not dismissible.
  useEffect(() => {
    if (!impacted) {
      setDismissed(false);
    }
  }, [impacted]);

  if (!impacted || dismissed) {
    return null;
  }

  const affectedCount = new Set([
    ...status.uncovered_bindings,
    ...status.degraded_bindings,
  ]).size;
  const countSuffix = affectedCount === 1 ? "one" : "other";
  const message =
    affectedCount > 0
      ? status.culprit_name !== null
        ? t(`secureInput.blockedWithCulprit_${countSuffix}`, {
            name: status.culprit_name,
            count: affectedCount,
          })
        : t(`secureInput.blockedNoCulprit_${countSuffix}`, {
            count: affectedCount,
          })
      : status.culprit_name !== null
        ? t("secureInput.recorderBlockedWithCulprit", {
            name: status.culprit_name,
          })
        : t("secureInput.recorderBlockedNoCulprit");

  return (
    <div className="w-full rounded-[7px] border-0 bg-[#fb8442]/[0.11] px-3 py-2.5 text-[#fb8442] shadow-none">
      <div className="flex items-center gap-3">
        <Warning className="h-5 w-5 shrink-0 text-[#fb8442]" />
        <p className="min-w-0 flex-1 text-sm font-medium leading-5">
          {message}
        </p>
        <div className="flex shrink-0 items-center gap-1">
          <button
            onClick={() => openUrl(SECURE_INPUT_HELP_URL)}
            className="cursor-pointer whitespace-nowrap rounded px-2 py-1.5 text-sm font-medium text-text hover:text-[#fb8442] focus:outline-none focus:ring-1 focus:ring-[#fb8442]"
          >
            <span className="flex items-center gap-1 border-b border-current leading-4">
              {t("secureInput.learnMore")}
              <ArrowSquareOut className="h-3.5 w-3.5" />
            </span>
          </button>
          <button
            onClick={() => setDismissed(true)}
            aria-label={t("secureInput.dismiss")}
            className="cursor-pointer rounded p-1.5 text-mid-gray hover:bg-[#fb8442]/15 hover:text-[#fb8442] focus:outline-none focus:ring-1 focus:ring-[#fb8442]"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  );
};

export default SecureInputWarning;
