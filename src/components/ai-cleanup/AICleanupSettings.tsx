/* eslint-disable i18next/no-literal-string */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Copy, FileText, Plus, UploadSimple, X } from "@phosphor-icons/react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type {
  AiCleanupConfiguration,
  AiCleanupHistoryEntry,
  AiCleanupStyle,
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
import { Alert } from "@/components/ui/Alert";
import { useIsLight } from "@/lib/utils/theme";

const MODELS: DropdownOption[] = [
  { value: "gemini-3.8-flash", label: "Gemini 3.8 Flash" },
  { value: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash Lite" },
  { value: "gemini-3.5-flash", label: "Gemini 3.5 Flash" },
  { value: "gemini-3.7-flash", label: "Gemini 3.7 Flash" },
  { value: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro Preview" },
].map((model) => ({
  ...model,
  icon: <img src="/icons/gemini.svg" alt="" className="size-4" />,
}));

const LOCAL_MODELS: DropdownOption[] = [
  {
    value: "prism-ml/Ternary-Bonsai-4B-mlx-2bit",
    label: "Ternary Bonsai 4B · 2-bit",
  },
  { value: "prism-ml/Bonsai-8B-mlx-2bit", label: "Bonsai 8B · 2-bit" },
  {
    value: "salohcin714/gemma-4-12B-it-2bit-mlx",
    label: "Gemma 4 12B IT · 2-bit",
  },
  {
    value: "lmstudio-community/gemma-4-12B-it-MLX-4bit",
    label: "Gemma 4 12B IT · 4-bit",
  },
  {
    value: "lmstudio-community/gemma-4-12B-it-MLX-8bit",
    label: "Gemma 4 12B IT · 8-bit",
  },
  { value: "mlx-community/Qwen3.8-27B-8bit", label: "Qwen3.8 27B · 8-bit" },
  { value: "Qwen/Qwen3.8-Flash-Next", label: "Qwen3.8 Flash Next" },
];

const THINKING_LABELS: Record<AiCleanupThinkingLevel, string> = {
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
};

const STYLES: AiCleanupStyle[] = [
  "default",
  "formal",
  "casual",
  "concise",
  "custom",
];

const STYLE_LABELS: Record<AiCleanupStyle, string> = {
  default: "Default",
  formal: "Formal",
  casual: "Casual",
  concise: "Concise",
  custom: "Custom",
};

const EMPTY_CONFIGURATION: AiCleanupConfiguration = {
  enabled: true,
  auto_enabled: false,
  model: "gemini-3.5-flash-lite",
  thinking_level: "minimal",
  style: "default",
  style_tone: "",
  custom_instruction: "",
  contexts: [],
};

export function AICleanupSettings() {
  const { settings, refreshSettings, updateSetting, isUpdating } =
    useSettings();
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
      style: settings.ai_cleanup_style ?? "default",
      style_tone: settings.ai_cleanup_style_tone ?? "",
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

  const copyCleanup = (text: string) => {
    navigator.clipboard.writeText(text).catch((error) => {
      console.error("Failed to copy cleanup:", error);
    });
  };

  // Local AI LLM: when on, every prompt below runs on the selected MLX
  // model instead of Gemini. Same prompts, same pipeline — only the
  // backend changes. Gemini settings stay exactly as they are.
  const localLlmEnabled = settings?.local_llm_enabled ?? false;
  const localLlmModel = settings?.local_llm_model ?? LOCAL_MODELS[0].value;
  const localModelLabel =
    LOCAL_MODELS.find((model) => model.value === localLlmModel)?.label ??
    localLlmModel;

  const selectLocalModel = (model: string) => {
    void updateSetting("local_llm_model", model);
  };

  const toggleLocalLlm = (enabled: boolean) => {
    void updateSetting("local_llm_enabled", enabled);
  };

  const saveApiKey = async (apiKey: string, successMessage?: string) => {
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
    if (successMessage) toast.success(successMessage);
  };

  const isLight = useIsLight();

  return (
    <div className="mx-auto w-full max-w-3xl space-y-8 pb-10">
      <header className="px-4 pt-2">
        <h1
          className={`text-[28px] font-normal tracking-tight ${isLight ? "text-stone-900" : "text-stone-50"}`}
        >
          AI clean up
        </h1>
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

      <SettingsGroup title="Local AI LLM">
        <ToggleSwitch
          checked={localLlmEnabled}
          onChange={toggleLocalLlm}
          label="Use Local LLM"
          description="Run every prompt on the selected MLX model on your Mac instead of Gemini."
          grouped
          isUpdating={isUpdating("local_llm_enabled")}
        />
        <SettingContainer
          title="Local model"
          description="Model used for all AI inference while Local AI LLM is on."
          grouped
          disabled={!localLlmEnabled}
        >
          <Dropdown
            options={LOCAL_MODELS}
            selectedValue={localLlmModel}
            onSelect={selectLocalModel}
            className="min-w-56"
          />
        </SettingContainer>
        {localLlmEnabled && (
          <p className="px-4 pb-3 text-xs leading-5 text-stone-500">
            Gemini is bypassed — the same prompts run on {localModelLabel}. The
            model stays loaded between requests.
          </p>
        )}
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
          title="Tone"
          description="Shape the voice of the cleaned prompt."
          grouped
        >
          <Dropdown
            options={STYLES.map((style) => ({
              value: style,
              label: STYLE_LABELS[style],
            }))}
            selectedValue={configuration.style}
            onSelect={(value) => update("style", value as AiCleanupStyle)}
            className="min-w-44"
          />
        </SettingContainer>
        {configuration.style === "custom" && (
          <div className="px-4 py-3">
            <div
              className={`rounded-[14px] p-4 ${isLight ? "bg-stone-100 border border-stone-200" : "bg-stone-800"}`}
            >
              <Textarea
                variant="inset"
                value={configuration.style_tone}
                onChange={(event) => update("style_tone", event.target.value)}
                maxLength={2000}
                className={`w-full !rounded-[12px] !text-[15px] ${isLight ? "!text-stone-900" : "!text-stone-100"}`}
                placeholder="What should the tone be? For example: warm and encouraging, like a mentor reviewing my work."
              />
            </div>
          </div>
        )}
        <div className="px-4 py-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p
                className={`text-sm font-medium ${isLight ? "text-stone-900" : "text-stone-100"}`}
              >
                Gemini API key
              </p>
              <p className="mt-1 text-xs leading-5 text-stone-500">
                Stored locally and used only for Gemini requests.
              </p>
            </div>
            <Badge
              variant={
                apiConfigured === null
                  ? "neutral"
                  : apiConfigured
                    ? "green"
                    : "rose"
              }
              className="shrink-0 whitespace-nowrap"
            >
              {apiConfigured === null
                ? "Checking…"
                : apiConfigured
                  ? "Configured"
                  : "API key not configured"}
            </Badge>
          </div>
          <Input
            type="text"
            value={apiKeyDraft}
            onChange={(event) => setApiKeyDraft(event.target.value)}
            onPaste={(event) => {
              const pasted = event.clipboardData.getData("text").trim();
              if (!pasted) return;
              // Pasting IS the intent to save — persist immediately instead
              // of waiting for a blur that may never come.
              event.preventDefault();
              setApiKeyDraft(pasted);
              void saveApiKey(pasted, "API_KEY pasted successfully");
            }}
            onBlur={() => void saveApiKey(apiKeyDraft)}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            disabled={apiKeySaving}
            placeholder={
              apiConfigured
                ? "Gemini API key configured"
                : "Enter Gemini API key"
            }
            autoComplete="off"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            className={`mt-3 w-full min-w-0 [-webkit-text-security:disc] ${isLight ? "text-stone-900" : "text-stone-100"}`}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title="Custom instruction"
        titleClassName={`text-sm font-normal normal-case tracking-normal ${isLight ? "text-stone-900" : "text-stone-100"}`}
        descriptionClassName="text-[13px] leading-5 text-stone-400"
        headerClassName="space-y-2 py-1"
        className="space-y-3"
        description="Add preferences without changing the protected system prompt."
      >
        <div className={`rounded-lg p-4 !shadow-none ${isLight ? "bg-white border border-stone-200/70" : "bg-stone-800"}`}>
          <Textarea
            variant="inset"
            value={configuration.custom_instruction}
            onChange={(event) => update("custom_instruction", event.target.value)}
            maxLength={4000}
            className={`w-full !rounded-lg !text-[15px] placeholder:text-stone-500 !shadow-none ${isLight ? "!bg-stone-100/80 !text-stone-900 !border-0" : "!text-stone-100"}`}
            placeholder="For example: Keep my tone direct and preserve code paths exactly."
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title="Context"
        description="Add writing samples or Markdown references Gemini should consider."
      >
        {configuration.contexts.map((context, index) => (
          <div
            key={index}
            className={`px-3 py-3 ${isLight ? "bg-stone-100 border-b border-stone-200" : "bg-stone-800"}`}
          >
            <div className="mb-2 flex items-center justify-between">
              <span
                className={`flex items-center gap-2 text-xs ${isLight ? "text-stone-600" : "text-stone-400"}`}
              >
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
                className={`flex size-7 items-center justify-center rounded-lg transition-colors ${isLight ? "text-stone-500 hover:bg-stone-200 hover:text-stone-700" : "text-stone-500 hover:bg-stone-700 hover:text-stone-200"}`}
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
            className={`inline-flex h-8 items-center gap-2 rounded-[10px] px-3 text-[13px] transition-colors disabled:opacity-40 ${isLight ? "bg-white border border-stone-200 text-stone-900 hover:bg-stone-100" : "bg-stone-700 text-stone-100 hover:bg-stone-600"}`}
          >
            <Plus size={14} /> Add context
          </button>
          <button
            type="button"
            onClick={() => fileInput.current?.click()}
            disabled={configuration.contexts.length >= 12}
            className={`inline-flex h-8 items-center gap-2 rounded-[10px] px-3 text-[13px] transition-colors disabled:opacity-40 ${isLight ? "bg-stone-100 border border-stone-200 text-stone-700 hover:bg-stone-200" : "bg-stone-850 text-stone-300 hover:bg-stone-700"}`}
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

      {saveError && (
        <Alert variant="error" className="mx-4">
          {saveError}
        </Alert>
      )}

      <SettingsGroup title="Recent cleanups">
        {history.length === 0 ? (
          <p className="px-4 py-6 text-sm text-stone-500">
            Cleaned prompts will appear here.
          </p>
        ) : (
          history.map((entry) => (
            <details key={entry.id} className="group px-4 py-3">
              <summary className="flex list-none items-center justify-between gap-4">
                <span
                  className={`truncate text-sm ${isLight ? "text-stone-900" : "text-stone-200"}`}
                >
                  {entry.output_text}
                </span>
                <span className="shrink-0 text-xs text-stone-500">
                  {new Date(entry.timestamp * 1000).toLocaleDateString()}
                </span>
              </summary>
              <div className="mt-3 grid gap-3 text-sm leading-6">
                <div>
                  <p className="mb-1 text-xs text-stone-500">Original</p>
                  <p
                    className={`whitespace-pre-wrap ${isLight ? "text-stone-600" : "text-stone-400"}`}
                  >
                    {entry.input_text}
                  </p>
                </div>
                <div>
                  <div className="mb-1 flex items-center justify-between gap-3">
                    <p className="text-xs text-stone-500">Clean prompt</p>
                    <button
                      type="button"
                      onClick={() => copyCleanup(entry.output_text)}
                      aria-label="Copy clean prompt"
                      title="Copy clean prompt"
                      className={`flex size-7 items-center justify-center rounded-md transition-colors ${isLight ? "text-stone-500 hover:bg-stone-100 hover:text-stone-900" : "text-stone-500 hover:bg-stone-700/60 hover:text-stone-100"}`}
                    >
                      <Copy size={14} />
                    </button>
                  </div>
                  <p
                    className={`whitespace-pre-wrap ${isLight ? "text-stone-900" : "text-stone-100"}`}
                  >
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
