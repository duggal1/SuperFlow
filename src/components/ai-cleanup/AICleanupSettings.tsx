/* eslint-disable i18next/no-literal-string */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileText, Plus, UploadSimple, X } from "@phosphor-icons/react";
import { commands } from "@/bindings";
import type {
  AiCleanupConfiguration,
  AiCleanupHistoryEntry,
  AiCleanupThinkingLevel,
} from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { ShortcutInput } from "@/components/settings/ShortcutInput";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { Textarea } from "@/components/ui/Textarea";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Badge } from "@/components/ui/Badge";
import { Input } from "@/components/ui/Input";

const MODELS: DropdownOption[] = [
  { value: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash Lite" },
  { value: "gemini-3.5-flash", label: "Gemini 3.5 Flash" },
  { value: "gemini-3.7-flash", label: "Gemini 3.7 Flash" },
  { value: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro Preview" },
].map((model) => ({
  ...model,
  icon: <img src="/icons/gemini.svg" alt="" className="size-4" />,
}));

const THINKING_LABELS: Record<AiCleanupThinkingLevel, string> = {
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
};

const EMPTY_CONFIGURATION: AiCleanupConfiguration = {
  enabled: true,
  auto_enabled: false,
  model: "gemini-3.5-flash-lite",
  thinking_level: "minimal",
  custom_instruction: "",
  contexts: [],
};

export function AICleanupSettings() {
  const { settings, refreshSettings } = useSettings();
  const [configuration, setConfiguration] =
    useState<AiCleanupConfiguration>(EMPTY_CONFIGURATION);
  const [history, setHistory] = useState<AiCleanupHistoryEntry[]>([]);
  const [apiConfigured, setApiConfigured] = useState<boolean | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [apiKeySaving, setApiKeySaving] = useState(false);
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const initialized = useRef(false);
  const lastSaved = useRef("");
  const latestConfiguration = useRef(configuration);
  const fileInput = useRef<HTMLInputElement>(null);

  latestConfiguration.current = configuration;

  useEffect(() => {
    if (!settings) return;
    const next = {
      enabled: settings.ai_cleanup_enabled ?? true,
      auto_enabled: settings.auto_ai_cleanup_enabled ?? false,
      model: settings.ai_cleanup_model ?? "gemini-3.5-flash-lite",
      thinking_level: settings.ai_cleanup_thinking_level ?? "minimal",
      custom_instruction: settings.ai_cleanup_custom_instruction ?? "",
      contexts: settings.ai_cleanup_contexts ?? [],
    } satisfies AiCleanupConfiguration;
    lastSaved.current = JSON.stringify(next);
    setConfiguration(next);
    initialized.current = true;
  }, [settings]);

  const loadStatus = useCallback(async () => {
    const [historyResult, configuredResult] = await Promise.all([
      commands.getAiCleanupHistory(8),
      commands.isGeminiApiConfigured(),
    ]);
    if (historyResult.status === "ok") setHistory(historyResult.data);
    if (configuredResult.status === "ok") {
      setApiConfigured(configuredResult.data);
    } else {
      setSaveError(configuredResult.error);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const persist = useCallback(
    async (next: AiCleanupConfiguration) => {
      const result = await commands.updateAiCleanupConfiguration(next);
      if (result.status === "error") {
        setSaveError(result.error);
        return;
      }
      lastSaved.current = JSON.stringify(next);
      setSaveError(null);
      if (JSON.stringify(latestConfiguration.current) === lastSaved.current) {
        await refreshSettings();
      }
    },
    [refreshSettings],
  );

  useEffect(() => {
    if (!initialized.current) return;
    if (JSON.stringify(configuration) === lastSaved.current) return;
    const timer = setTimeout(() => void persist(configuration), 350);
    return () => clearTimeout(timer);
  }, [configuration, persist]);

  const thinkingOptions = useMemo<DropdownOption[]>(() => {
    const levels: AiCleanupThinkingLevel[] =
      configuration.model === "gemini-3.1-pro-preview"
        ? ["low", "medium", "high"]
        : ["minimal", "low", "medium", "high"];
    return levels.map((level) => ({
      value: level,
      label: THINKING_LABELS[level],
    }));
  }, [configuration.model]);

  const update = <K extends keyof AiCleanupConfiguration>(
    key: K,
    value: AiCleanupConfiguration[K],
  ) => setConfiguration((current) => ({ ...current, [key]: value }));

  const selectModel = (model: string) => {
    setConfiguration((current) => ({
      ...current,
      model,
      thinking_level:
        model === "gemini-3.1-pro-preview" &&
        current.thinking_level === "minimal"
          ? "low"
          : current.thinking_level,
    }));
  };

  const uploadMarkdown = async (files: FileList | null) => {
    if (!files) return;
    const additions = await Promise.all(
      Array.from(files).map((file) => file.text()),
    );
    update("contexts", [...configuration.contexts, ...additions].slice(0, 12));
    if (fileInput.current) fileInput.current.value = "";
  };

  const saveApiKey = async (apiKey: string) => {
    if (!apiKey.trim()) return;
    setApiKeySaving(true);
    const result = await commands.setGeminiApiKey(apiKey);
    setApiKeySaving(false);
    if (result.status === "error") {
      setSaveError(result.error);
      return;
    }
    setApiKeyDraft("");
    setSaveError(null);
    await loadStatus();
  };

  return (
    <div className="mx-auto w-full max-w-3xl space-y-8 pb-10">
      <header className="px-4 pt-2">
        <h1 className="text-[28px] font-normal tracking-tight text-stone-50">
          AI clean up
        </h1>
        <p className="mt-2 text-[15px] leading-6 text-stone-400">
          Turn selected text into clear, focused prompts with one shortcut.
        </p>
      </header>

      <SettingsGroup title="Workflow">
        <ToggleSwitch
          checked={configuration.enabled}
          onChange={(enabled) => update("enabled", enabled)}
          label="AI clean up"
          description="Use the shortcut to clean selected text with Gemini."
          grouped
        />
        <ShortcutInput
          shortcutId="ai_cleanup"
          grouped
          disabled={!configuration.enabled}
        />
        <ToggleSwitch
          checked={configuration.auto_enabled}
          onChange={(enabled) => update("auto_enabled", enabled)}
          label="Auto AI Cleanup"
          description="Clean every completed transcription before it is pasted."
          grouped
        />
      </SettingsGroup>

      <SettingsGroup title="Model">
        <SettingContainer
          title="Gemini model"
          description="Choose the model used for prompt cleanup."
          grouped
        >
          <Dropdown
            options={MODELS}
            selectedValue={configuration.model}
            onSelect={selectModel}
            className="min-w-56"
          />
        </SettingContainer>
        <SettingContainer
          title="Thinking"
          description="Control the reasoning depth used for each cleanup."
          grouped
        >
          <Dropdown
            options={thinkingOptions}
            selectedValue={configuration.thinking_level}
            onSelect={(value) =>
              update("thinking_level", value as AiCleanupThinkingLevel)
            }
            className="min-w-44"
          />
        </SettingContainer>
        <SettingContainer
          title="Gemini API key"
          description="Stored locally and used only for Gemini requests."
          grouped
        >
          <div className="flex items-center gap-2">
            <Input
              type="password"
              value={apiKeyDraft}
              onChange={(event) => setApiKeyDraft(event.target.value)}
              onBlur={() => void saveApiKey(apiKeyDraft)}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
              }}
              disabled={apiKeySaving}
              placeholder={
                apiConfigured
                  ? "Saved in macOS Keychain"
                  : "Enter Gemini API key"
              }
              autoComplete="off"
              className="min-w-[320px] text-stone-100"
            />
            <Badge variant={apiConfigured ? "green" : "rose"}>
              {apiConfigured ? "Configured" : "Missing key"}
            </Badge>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title="Custom instruction"
        titleClassName="text-xs font-normal text-stone-100 normal-case tracking-wide"
        description="Add preferences without changing the protected system prompt."
      >
        <div className="rounded-[12px] bg-stone-800 p-3">
          <Textarea
            variant="inset"
            value={configuration.custom_instruction}
            onChange={(event) =>
              update("custom_instruction", event.target.value)
            }
            maxLength={4000}
            className="w-full !rounded-[10px] !text-stone-100"
            placeholder="For example: Keep my tone direct and preserve code paths exactly."
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title="Context"
        description="Add writing samples or Markdown references Gemini should consider."
      >
        {configuration.contexts.map((context, index) => (
          <div key={index} className="bg-stone-800 px-3 py-3">
            <div className="mb-2 flex items-center justify-between">
              <span className="flex items-center gap-2 text-xs text-stone-400">
                <FileText size={14} /> Context {index + 1}
              </span>
              <button
                type="button"
                onClick={() =>
                  update(
                    "contexts",
                    configuration.contexts.filter((_, item) => item !== index),
                  )
                }
                aria-label={`Remove context ${index + 1}`}
                className="flex size-7 items-center justify-center rounded-lg text-stone-500 transition-colors hover:bg-stone-700 hover:text-stone-200"
              >
                <X size={14} />
              </button>
            </div>
            <Textarea
              variant="inset"
              value={context}
              maxLength={12000}
              onChange={(event) => {
                const contexts = [...configuration.contexts];
                contexts[index] = event.target.value;
                update("contexts", contexts);
              }}
              className="w-full"
              placeholder="Paste a writing sample or useful reference."
            />
          </div>
        ))}
        <div className="flex items-center gap-2 px-4 py-3">
          <button
            type="button"
            onClick={() => update("contexts", [...configuration.contexts, ""])}
            disabled={configuration.contexts.length >= 12}
            className="inline-flex h-8 items-center gap-2 rounded-[10px] bg-stone-700 px-3 text-[13px] text-stone-100 transition-colors hover:bg-stone-600 disabled:opacity-40"
          >
            <Plus size={14} /> Add context
          </button>
          <button
            type="button"
            onClick={() => fileInput.current?.click()}
            disabled={configuration.contexts.length >= 12}
            className="inline-flex h-8 items-center gap-2 rounded-[10px] bg-stone-850 px-3 text-[13px] text-stone-300 transition-colors hover:bg-stone-700 disabled:opacity-40"
          >
            <UploadSimple size={14} /> Upload Markdown
          </button>
          <input
            ref={fileInput}
            type="file"
            accept=".md,.markdown,text/markdown"
            multiple
            className="hidden"
            onChange={(event) => void uploadMarkdown(event.target.files)}
          />
        </div>
      </SettingsGroup>

      {saveError && <p className="px-4 text-sm text-rose-400">{saveError}</p>}

      <SettingsGroup title="Recent cleanups">
        {history.length === 0 ? (
          <p className="px-4 py-6 text-sm text-stone-500">
            Cleaned prompts will appear here.
          </p>
        ) : (
          history.map((entry) => (
            <details key={entry.id} className="group px-4 py-3">
              <summary className="flex list-none items-center justify-between gap-4">
                <span className="truncate text-sm text-stone-200">
                  {entry.output_text}
                </span>
                <span className="shrink-0 text-xs text-stone-500">
                  {new Date(entry.timestamp * 1000).toLocaleDateString()}
                </span>
              </summary>
              <div className="mt-3 grid gap-3 text-sm leading-6">
                <div>
                  <p className="mb-1 text-xs text-stone-500">Original</p>
                  <p className="whitespace-pre-wrap text-stone-400">
                    {entry.input_text}
                  </p>
                </div>
                <div>
                  <p className="mb-1 text-xs text-stone-500">Clean prompt</p>
                  <p className="whitespace-pre-wrap text-stone-100">
                    {entry.output_text}
                  </p>
                </div>
              </div>
            </details>
          ))
        )}
      </SettingsGroup>
    </div>
  );
}
