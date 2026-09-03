import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/Input";
import { IOSSpinner } from "@/components/shared/global-spinner";
import { useIsLight } from "@/lib/utils/theme";

type Provider = "google" | "microsoft";

interface OAuthSetupFormProps {
  provider: Provider;
  /** Called after credentials were saved so the page can refresh state. */
  onSaved: () => void;
}

/**
 * Bring-your-own OAuth setup. The user creates their own Google Cloud /
 * Microsoft Entra OAuth app and pastes its client IDs here, so the consent
 * screen shows *their* app — no SuperFlow-owned credentials are involved.
 * Credentials persist in the local settings store; user tokens stay in the
 * OS Keychain.
 */
export const OAuthSetupForm: React.FC<OAuthSetupFormProps> = ({
  provider,
  onSaved,
}) => {
  const { t } = useTranslation();
  const isLight = useIsLight();
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isGoogle = provider === "google";

  const labelClass = "text-[12px] font-normal tracking-tight text-stone-200";
  const stepClass = "text-[12px] leading-5 tracking-tight text-stone-200";
  const sectionClass = "space-y-3";
  const buttonClass = isLight
    ? "inline-flex h-7 cursor-pointer items-center justify-center gap-1.5 rounded-md bg-stone-900 px-3 text-sm font-normal tracking-tight leading-none text-white transition-colors duration-150 hover:bg-stone-950 disabled:pointer-events-none disabled:bg-stone-100 disabled:text-stone-400"
    : "inline-flex h-7 cursor-pointer items-center justify-center gap-1.5 rounded-md bg-stone-100 px-3 text-sm font-normal tracking-tight leading-none text-stone-900 transition-colors duration-150 hover:bg-white disabled:pointer-events-none disabled:bg-stone-800 disabled:text-stone-600";

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      if (isGoogle) {
        await invoke("integrations_save_google_credentials", {
          clientId,
          clientSecret: clientSecret || null,
        });
      } else {
        await invoke("integrations_save_microsoft_credentials", {
          clientId,
        });
      }
      setClientId("");
      setClientSecret("");
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div id={`setup-${provider}`} className={sectionClass}>
      <ol className="list-decimal space-y-1 pl-5 text-stone-200">
        {isGoogle ? (
          <>
            <li className={stepClass}>Create Google Cloud OAuth app</li>
            <li className={stepClass}>Add redirect URI from docs</li>
            <li className={stepClass}>Paste Client ID and Secret below</li>
          </>
        ) : (
          <>
            <li className={stepClass}>Create Entra OAuth app</li>
            <li className={stepClass}>Add redirect URI from docs</li>
            <li className={stepClass}>Paste Client ID below</li>
          </>
        )}
      </ol>

      <div className="grid grid-cols-1 gap-3">
        <label className="space-y-1.5">
          <span className={labelClass}>{t("agents.clientId")}</span>
          <Input
            type="text"
            value={clientId}
            onChange={(e) => setClientId(e.target.value)}
            placeholder={
              isGoogle
                ? "1234567890-abc123.apps.googleusercontent.com"
                : "00000000-0000-0000-0000-000000000000"
            }
            spellCheck={false}
            autoComplete="off"
            className="w-full"
          />
        </label>
        {isGoogle && (
          <label className="space-y-1.5">
            <span className={labelClass}>{t("agents.clientSecret")}</span>
            <Input
              type="password"
              value={clientSecret}
              onChange={(e) => setClientSecret(e.target.value)}
              placeholder="GOCSPX-…"
              autoComplete="off"
              className="w-full"
            />
          </label>
        )}
      </div>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={saving || !clientId.trim()}
          className={buttonClass}
        >
          {saving && (
            <IOSSpinner size={12} color={isLight ? "#ffffff" : "#0c0a09"} />
          )}
          {saving ? t("agents.saving") : t("agents.saveCredentials")}
        </button>
        {error && (
          <span className="text-[13px] tracking-tight text-red-500">
            {error}
          </span>
        )}
      </div>
    </div>
  );
};
