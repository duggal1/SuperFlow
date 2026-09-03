import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { IOSSpinner } from "@/components/shared/global-spinner";
import { Sonner, type SonnerState } from "@/components/toast";
import { useTranslation } from "react-i18next";
import { OAuthSetupForm } from "./OAuthSetupForm";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon, ArrowUp01Icon } from "@hugeicons/core-free-icons";
import { useIsLight } from "@/lib/utils/theme";
import { TtsVoiceSection } from "./agent-picker/tts-voice-section";
import { ErrorBoundary } from "../ErrorBoundary";

type Provider = "google" | "microsoft";

interface StackDef {
  provider: Provider;
  name: string;
  description: string;
  icon: string;
  color: "orange" | "blue";
  items: { id: string; name: string; icon: string }[];
}

const STACKS: StackDef[] = [
  {
    provider: "google",
    name: "Google Stack",
    description: "Connect Gmail, Calendar, Drive and Docs in one step",
    icon: "/icons/gmail.svg",
    color: "orange",
    items: [
      { id: "gmail", name: "Gmail", icon: "/icons/gmail.svg" },
      { id: "google-calendar", name: "Google Calendar", icon: "/icons/gmail.svg" },
      { id: "google-drive", name: "Google Drive", icon: "/icons/gmail.svg" },
      { id: "google-docs", name: "Google Docs", icon: "/icons/gmail.svg" },
    ],
  },
  {
    provider: "microsoft",
    name: "Microsoft Stack",
    description: "Connect Outlook, Calendar and OneDrive in one step",
    icon: "/icons/microsoft-outlook.svg",
    color: "blue",
    items: [
      { id: "outlook", name: "Outlook", icon: "/icons/microsoft-outlook.svg" },
      { id: "outlook-calendar", name: "Outlook Calendar", icon: "/icons/microsoft-outlook.svg" },
      { id: "onedrive", name: "OneDrive", icon: "/icons/microsoft-outlook.svg" },
    ],
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
  const [expanded, setExpanded] = useState<Record<Provider, boolean>>({
    google: false,
    microsoft: false,
  });
  const [sonner, setSonner] = useState<SonnerState | null>(null);
  const [creds, setCreds] = useState<{ google: boolean; microsoft: boolean } | null>(null);

  const mountedRef = React.useRef(true);
  React.useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refreshCredentials = useCallback(async () => {
    try {
      const c = await invoke<{ google: boolean; microsoft: boolean }>("integrations_credentials_status");
      if (!mountedRef.current) return;
      setCreds(c);
    } catch {
      if (!mountedRef.current) return;
      setCreds({ google: true, microsoft: true });
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const [google, microsoft] = await Promise.all([
        invoke<boolean>("google_status"),
        invoke<boolean>("microsoft_status"),
      ]);
      if (!mountedRef.current) return;
      setConnected({ google, microsoft });
    } catch {
      if (!mountedRef.current) return;
      setConnected({ google: false, microsoft: false });
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    void refreshCredentials();
  }, [refreshStatus, refreshCredentials]);

  const handleToggle = useCallback(
    async (provider: Provider, action: "connect" | "disconnect", label: string) => {
      if (!mountedRef.current) return;
      setPending((prev) => ({ ...prev, [provider]: action }));
      setSonner({ kind: "loading", message: action === "connect" ? `Connecting ${label}…` : `Disconnecting ${label}…` });
      try {
        await invoke(`${provider}_${action}`);
        if (!mountedRef.current) return;
        setSonner({ kind: "success", message: action === "connect" ? `${label} connected` : `${label} disconnected` });
      } catch (error) {
        if (!mountedRef.current) return;
        setSonner({ kind: "error", message: `${label} ${action} failed: ${String(error)}` });
      } finally {
        if (!mountedRef.current) return;
        setPending((prev) => ({ ...prev, [provider]: null }));
        void refreshStatus();
      }
    },
    [refreshStatus],
  );

  return (
    <main className={`mx-auto flex min-h-full w-full max-w-5xl flex-col px-6 pb-16 pt-6 ${isLight ? "bg-stone-100" : "bg-stone-900"}`}>
      <header className="mb-6">
        <h1 className={`text-[22px] font-normal tracking-tight ${isLight ? "text-stone-900" : "text-stone-100"}`}>Agents</h1>
        <p className={`mt-1 max-w-xl text-[13px] leading-5 ${isLight ? "text-stone-600" : "text-stone-400"}`}>The most powerful way to do professional work with voice.</p>
      </header>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {STACKS.map((stack) => {
          const isConnected = connected[stack.provider] === true;
          const isBusy = pending[stack.provider] !== null;
          const isExpanded = expanded[stack.provider];

          return (
            <div
              key={stack.provider}
              className={`flex flex-col gap-4 rounded-xl px-5 py-5 ${isLight ? "border border-stone-200/70 bg-white" : "bg-stone-800"}`}
            >
              <div className="flex items-start justify-between gap-4">
                <div className="flex items-center gap-3">
                  <div className={`flex size-9 shrink-0 items-center justify-center rounded-lg ${isLight ? "bg-stone-50" : "bg-white/[0.06]"}`}>
                    <img src={stack.icon} alt="" className="size-6" draggable={false} />
                  </div>
                  <div className="min-w-0">
                    <p className={`text-[15px] font-normal tracking-tight ${isLight ? "text-stone-900" : "text-stone-100"}`}>{stack.name}</p>
                    <p className={`mt-0.5 text-[12px] leading-4 ${isLight ? "text-stone-500" : "text-stone-400"}`}>{stack.description}</p>
                  </div>
                </div>
                <Badge variant={stack.color === "orange" ? "orange" : "blue"}>{stack.color === "orange" ? "Google" : "Microsoft"}</Badge>
              </div>

              <div className="flex items-center justify-between gap-3 pt-1">
                <div className="flex items-center gap-2">
                  {connected[stack.provider] === null ? (
                    <IOSSpinner size={14} />
                  ) : isConnected ? (
                    <Badge variant="green">
                      <span className="size-1.5 rounded-full bg-green-500" />
                      Connected
                    </Badge>
                  ) : (
                    <Badge variant="rose">Not connected</Badge>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setExpanded((p) => ({ ...p, [stack.provider]: !p[stack.provider] }))}
                    className={`inline-flex items-center gap-1 text-xs font-normal underline-offset-4 hover:underline ${isLight ? "text-stone-600 hover:text-stone-900" : "text-stone-400 hover:text-stone-100"}`}
                  >
                    View all
                    <HugeiconsIcon icon={isExpanded ? ArrowUp01Icon : ArrowDown01Icon} size={12} />
                  </button>
                  {isConnected ? (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handleToggle(stack.provider, "disconnect", stack.name)}
                      disabled={isBusy}
                    >
                      {pending[stack.provider] === "disconnect" && <IOSSpinner size={12} />}
                      Disconnect
                    </Button>
                  ) : creds !== null && !creds[stack.provider] ? (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => document.getElementById(`setup-${stack.provider}`)?.scrollIntoView({ behavior: "smooth", block: "center" })}
                    >
                      Connect
                    </Button>
                  ) : (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handleToggle(stack.provider, "connect", stack.name)}
                      disabled={isBusy}
                    >
                      {pending[stack.provider] === "connect" && <IOSSpinner size={12} />}
                      Connect
                    </Button>
                  )}
                </div>
              </div>

              {isExpanded && (
                <div className="grid grid-cols-1 gap-2 pt-2">
                  {stack.items.map((item) => (
                    <div
                      key={item.id}
                      className={`flex items-center justify-between rounded-lg px-3 py-2.5 ${isLight ? "bg-stone-50" : "bg-[#32302d]"}`}
                    >
                      <div className="flex items-center gap-2.5">
                        <img src={item.icon} alt="" className="size-4" draggable={false} />
                        <span className={`text-xs font-normal ${isLight ? "text-stone-900" : "text-stone-100"}`}>{item.name}</span>
                      </div>
                      {isConnected ? (
                        <span className={`text-[11px] ${isLight ? "text-emerald-600" : "text-emerald-500"}`}>Included</span>
                      ) : (
                        <Badge variant="rose" className="text-[10px]">
                          Not connected
                        </Badge>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {creds !== null && (!creds.google || !creds.microsoft) && (
        <section className="mt-8 grid grid-cols-1 gap-3 lg:grid-cols-2">
          {!creds.google && (
            <div id="setup-google" className={`rounded-xl p-5 ${isLight ? "border border-stone-200/70 bg-white" : "bg-stone-800"}`}>
              <h3 className={`text-sm font-normal ${isLight ? "text-stone-900" : "text-stone-100"}`}>Bring your own Google keys</h3>
              <p className={`mt-1 text-xs leading-4 ${isLight ? "text-stone-500" : "text-stone-400"}`}>Connect to your own Google app. Create your own OAuth keys — quick and private.</p>
              <div className="mt-4">
                <OAuthSetupForm
                  provider="google"
                  onSaved={() => {
                    void refreshCredentials();
                    setSonner({ kind: "success", message: "Credentials saved" });
                  }}
                />
              </div>
            </div>
          )}
          {!creds.microsoft && (
            <div id="setup-microsoft" className={`rounded-xl p-5 ${isLight ? "border border-stone-200/70 bg-white" : "bg-stone-800"}`}>
              <h3 className={`text-sm font-normal ${isLight ? "text-stone-900" : "text-stone-100"}`}>Bring your own Microsoft keys</h3>
              <p className={`mt-1 text-xs leading-4 ${isLight ? "text-stone-500" : "text-stone-400"}`}>Connect to your own Microsoft app. Create your own OAuth keys — quick and private.</p>
              <div className="mt-4">
                <OAuthSetupForm
                  provider="microsoft"
                  onSaved={() => {
                    void refreshCredentials();
                    setSonner({ kind: "success", message: "Credentials saved" });
                  }}
                />
              </div>
            </div>
          )}
        </section>
      )}

      <ErrorBoundary context="TTS Voice">
        <React.Suspense fallback={null}>
          <div className="mt-8">
            <TtsVoiceSection />
          </div>
        </React.Suspense>
      </ErrorBoundary>

      <Sonner sonner={sonner} />
    </main>
  );
};
