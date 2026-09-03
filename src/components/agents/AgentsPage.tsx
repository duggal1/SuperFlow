import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Badge } from "@/components/ui/Badge";
import { IOSSpinner } from "@/components/shared/global-spinner";
import { Sonner, type SonnerState } from "@/components/toast";
import { useIsLight } from "@/lib/utils/theme";
import { useTranslation } from "react-i18next";

type Provider = "google" | "microsoft";

interface IntegrationDef {
  id: string;
  provider: Provider;
  name: string;
  description: string;
  icon: string;
}

// Every integration exposed by the tori-integrations backend. Google and
// Microsoft each share one OAuth connection per provider (account "default"),
// so all cards of a provider connect/disconnect together.
const INTEGRATIONS: IntegrationDef[] = [
  {
    id: "gmail",
    provider: "google",
    name: "Gmail",
    description: "Read, search, and send email with your voice",
    icon: "/icons/gmail.svg",
  },
  {
    id: "google-calendar",
    provider: "google",
    name: "Google Calendar",
    description: "Create and manage calendar events hands-free",
    icon: "/icons/gmail.svg",
  },
  {
    id: "google-drive",
    provider: "google",
    name: "Google Drive",
    description: "List and upload files the app has access to",
    icon: "/icons/gmail.svg",
  },
  {
    id: "google-docs",
    provider: "google",
    name: "Google Docs",
    description: "Create documents and insert text instantly",
    icon: "/icons/gmail.svg",
  },
  {
    id: "outlook",
    provider: "microsoft",
    name: "Outlook",
    description: "Read and send mail through Microsoft Graph",
    icon: "/icons/microsoft-outlook.svg",
  },
  {
    id: "outlook-calendar",
    provider: "microsoft",
    name: "Outlook Calendar",
    description: "List and create events across your calendars",
    icon: "/icons/microsoft-outlook.svg",
  },
  {
    id: "onedrive",
    provider: "microsoft",
    name: "OneDrive",
    description: "Browse your root folder and upload files",
    icon: "/icons/microsoft-outlook.svg",
  },
];

type PendingAction = "connect" | "disconnect" | null;

export const AgentsPage: React.FC = () => {
  const { t } = useTranslation();
  const isLight = useIsLight();
  const [connected, setConnected] = useState<Record<Provider, boolean | null>>({
    google: null,
    microsoft: null,
  });
  const [pending, setPending] = useState<Record<Provider, PendingAction>>({
    google: null,
    microsoft: null,
  });
  const [sonner, setSonner] = useState<SonnerState | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const [google, microsoft] = await Promise.all([
        invoke<boolean>("google_status"),
        invoke<boolean>("microsoft_status"),
      ]);
      setConnected({ google, microsoft });
    } catch {
      setConnected({ google: false, microsoft: false });
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const handleToggle = useCallback(
    async (integration: IntegrationDef, action: "connect" | "disconnect") => {
      const provider = integration.provider;
      setPending((prev) => ({ ...prev, [provider]: action }));
      setSonner({
        kind: "loading",
        message:
          action === "connect"
            ? `Connecting to ${integration.name}…`
            : `Disconnecting ${integration.name}…`,
      });
      try {
        await invoke(`${provider}_${action}`);
        setSonner({
          kind: "success",
          message:
            action === "connect"
              ? `${integration.name} connected — tokens are stored in your Keychain`
              : `${integration.name} disconnected`,
        });
      } catch (error) {
        setSonner({
          kind: "error",
          message: `${integration.name} ${action} failed: ${String(error)}`,
        });
      } finally {
        setPending((prev) => ({ ...prev, [provider]: null }));
        void refreshStatus();
      }
    },
    [refreshStatus],
  );

  // Sapphire tokens — stone-only, one border on the card edge, separation
  // via bg-color steps, radius trimmed one notch down.
  const cardClass = isLight
    ? "rounded-lg border border-stone-200/70 bg-white"
    : "rounded-lg bg-stone-800";
  const iconTileClass = isLight ? "bg-stone-50" : "bg-stone-900";
  const titleClass = isLight ? "text-stone-900" : "text-stone-100";
  const descriptionClass = isLight ? "text-stone-500" : "text-stone-400";

  const buttonBase =
    "inline-flex h-7 cursor-pointer items-center justify-center gap-1.5 rounded-md px-3 text-sm font-normal tracking-tight leading-none transition-colors duration-150";
  const primaryButtonClass = isLight
    ? "bg-stone-900 text-white hover:bg-stone-950"
    : "bg-stone-100 text-stone-900 hover:bg-white";
  const secondaryButtonClass = isLight
    ? "bg-stone-100 text-stone-900 hover:bg-stone-50"
    : "bg-stone-700 text-stone-100 hover:bg-stone-600";
  const disabledButtonClass = isLight
    ? "pointer-events-none bg-stone-100 text-stone-400"
    : "pointer-events-none bg-stone-800 text-stone-600";

  return (
    <main className="mx-auto flex min-h-full w-full max-w-4xl flex-col px-6 pb-16 pt-3">
      <header className="mt-5">
        <h1 className={`text-[22px] font-normal tracking-tight ${titleClass}`}>
          {t("agents.title")}
        </h1>
        <p
          className={`mt-1 text-[14px] leading-normal tracking-normal ${descriptionClass}`}
        >
          {t("agents.subtitle")}
        </p>
      </header>

      <div className="mt-8 flex flex-col gap-3">
        {INTEGRATIONS.map((integration) => {
          const provider = integration.provider;
          const isConnecting = pending[provider] === "connect";
          const isDisconnecting = pending[provider] === "disconnect";
          const isBusy = pending[provider] !== null;
          const isConnected = connected[provider] === true;

          return (
            <div
              key={integration.id}
              className={`flex items-start justify-between gap-5 px-6 py-7 ${cardClass}`}
            >
              <div className="flex min-w-0 items-start gap-5">
                <div
                  className={`flex size-11 shrink-0 items-center justify-center rounded-md ${iconTileClass}`}
                >
                  <img
                    src={integration.icon}
                    alt=""
                    className="size-7"
                    draggable={false}
                  />
                </div>
                <div className="min-w-0 space-y-1.5">
                  <div className="flex items-center gap-2">
                    <h2
                      className={`text-[17px] font-normal tracking-normal ${titleClass}`}
                    >
                      {integration.name}
                    </h2>
                    <Badge variant={provider === "google" ? "sky" : "blue"}>
                      {t(
                        provider === "google"
                          ? "agents.google"
                          : "agents.microsoft",
                      )}
                    </Badge>
                  </div>
                  <p
                    className={`text-[14px] leading-normal tracking-normal ${descriptionClass}`}
                  >
                    {integration.description}
                  </p>
                  <div className="pt-0.5">
                    {connected[provider] === null ? (
                      <IOSSpinner size={14} />
                    ) : isConnected ? (
                      <Badge variant="green">
                        <span className="size-1.5 rounded-full bg-green-500" />
                        {t("agents.connected")}
                      </Badge>
                    ) : (
                      <Badge variant="neutral">
                        <span className="size-1.5 rounded-full bg-neutral-400" />
                        {t("agents.notConnected")}
                      </Badge>
                    )}
                  </div>
                </div>
              </div>

              <div className="flex shrink-0 items-center gap-2 pt-1.5">
                {isConnected ? (
                  <button
                    type="button"
                    onClick={() => handleToggle(integration, "disconnect")}
                    disabled={isBusy}
                    className={`${buttonBase} ${
                      isDisconnecting
                        ? disabledButtonClass
                        : secondaryButtonClass
                    }`}
                  >
                    {isDisconnecting && (
                      <IOSSpinner
                        size={12}
                        color={isLight ? "#57534e" : "#d6d3d1"}
                      />
                    )}
                    {t("agents.disconnect")}
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => handleToggle(integration, "connect")}
                    disabled={isBusy}
                    className={`${buttonBase} ${
                      isConnecting ? disabledButtonClass : primaryButtonClass
                    }`}
                  >
                    {isConnecting && (
                      <IOSSpinner
                        size={12}
                        color={isLight ? "#ffffff" : "#0c0a09"}
                      />
                    )}
                    {t("agents.connect")}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      <Sonner sonner={sonner} />
    </main>
  );
};
