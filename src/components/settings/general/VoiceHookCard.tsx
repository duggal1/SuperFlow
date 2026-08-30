import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Input } from "../../ui/Input";
import { Badge } from "../../ui/Badge";
import { Dropdown, type DropdownOption } from "../../ui/Dropdown";
import { ShortcutInput } from "../ShortcutInput";

const MAX_HOOK_CHARS = 80;

export const VoiceHookCard: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const stored = getSetting("voice_command_hook") ?? "Hey SuperFlow";
  const busy = isUpdating("voice_command_hook");

  const TONE_OPTIONS: DropdownOption[] = [
    { value: "own", label: "Keep my voice" },
    { value: "professional", label: "Professional" },
    { value: "concise", label: "Concise" },
    { value: "informal", label: "Informal" },
    { value: "humanized", label: "Humanized" },
  ];
  const storedTone = getSetting("hey_superflow_tone") ?? "own";
  const toneBusy = isUpdating("hey_superflow_tone");

  const [draft, setDraft] = useState(stored);
  const [error, setError] = useState<string | null>(null);
  const lastStored = useRef(stored);
  const lastSaved = useRef(stored);

  // Sync when store changes externally (e.g. after backend normalize or reload)
  useEffect(() => {
    if (stored !== lastStored.current) {
      lastStored.current = stored;
      lastSaved.current = stored;
      setDraft(stored);
      setError(null);
    }
  }, [stored]);

  // Debounced persist
  useEffect(() => {
    if (draft === lastSaved.current) return;
    const normalizedPreview = draft.split(/\s+/).filter(Boolean).join(" ");

    // Don't validate empty while user is typing, just wait for blur/save attempt.
    // Validate on debounce to surface error.
    const timer = setTimeout(async () => {
      const normalized = draft.split(/\s+/).filter(Boolean).join(" ");
      if (!normalized) {
        setError(t("settings.general.superWhisper.emptyError"));
        return;
      }
      if (normalized.length > MAX_HOOK_CHARS) {
        setError(t("settings.general.superWhisper.tooLongError"));
        return;
      }
      try {
        await updateSetting("voice_command_hook", normalized as never);
        lastSaved.current = normalized;
        lastStored.current = normalized;
        setError(null);
        if (normalized !== draft) setDraft(normalized);
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        setError(message);
        toast.error(message);
      }
    }, 420);
    return () => clearTimeout(timer);
  }, [draft, t, updateSetting]);

  const normalizedPreview = draft.split(/\s+/).filter(Boolean).join(" ") || stored;
  const remaining = MAX_HOOK_CHARS - draft.length;
  const overLimit = draft.length > MAX_HOOK_CHARS;
  const empty = draft.trim().length === 0;

  return (
    <SettingsGroup title={t("settings.general.superWhisper.title")}>
      <div className="px-4 py-3.5">
        {/* AI Hotkey on top — same UX as standard, never same */}
        <div className="flex flex-col gap-2">
          <p className="text-sm font-medium tracking-tight text-stone-100">
            {t("settings.general.superWhisper.aiHotkeyLabel", { defaultValue: "AI hotkey" })}
          </p>
          <ShortcutInput shortcutId="transcribe_with_ai" grouped={false} />
        </div>

        {/* Hook row: label + live badge mirror */}
        <div className="mt-4 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-sm font-medium tracking-tight text-stone-100">
              {t("settings.general.superWhisper.hookLabel")}
            </p>
          </div>
          <Badge
            variant={empty ? "rose" : "sky"}
            className="hidden shrink-0 whitespace-nowrap sm:inline-flex"
          >
            {empty ? t("settings.general.superWhisper.emptyBadge") : normalizedPreview}
          </Badge>
        </div>

        {/* Ultra-clean input: stone-900 fill, hairline, blue focus */}
        <div className="mt-3">
          <Input
            type="text"
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value);
              if (error) setError(null);
            }}
            onBlur={() => {
              // Commit on blur immediately if debounced save hasn't fired yet
              const normalized = draft.split(/\s+/).filter(Boolean).join(" ");
              if (normalized && normalized !== lastSaved.current && normalized.length <= MAX_HOOK_CHARS) {
                void (async () => {
                  try {
                    await updateSetting("voice_command_hook", normalized as never);
                    lastSaved.current = normalized;
                    lastStored.current = normalized;
                    setError(null);
                    if (normalized !== draft) setDraft(normalized);
                  } catch (e) {
                    const message = e instanceof Error ? e.message : String(e);
                    setError(message);
                    toast.error(message);
                  }
                })();
              } else if (!normalized) {
                setError(t("settings.general.superWhisper.emptyError"));
              }
            }}
            maxLength={MAX_HOOK_CHARS + 10}
            disabled={busy}
            placeholder="Hey SuperFlow"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            className="w-full text-[15px] font-medium tracking-tight"
          />
          <div className="mt-1.5 flex justify-end">
            <span className={`shrink-0 text-xs tabular-nums ${overLimit ? "text-rose-400" : remaining < 12 ? "text-amber-400" : "text-stone-500"}`}>
              {draft.length}/{MAX_HOOK_CHARS}
            </span>
          </div>
        </div>

        {/* Example — ultra subtle, same surface, dotted underline hint */}
        <div className="mt-3 rounded-lg bg-stone-900/70 px-3 py-2.5">
          <p className="text-xs font-medium tracking-wide text-stone-400">
            {t("settings.general.superWhisper.exampleLabel")}
          </p>
          <p className="mt-1 text-sm leading-6 text-stone-200 [mask-image:linear-gradient(to_bottom_right,black_60%,transparent_100%)] [-webkit-mask-image:linear-gradient(to_bottom_right,black_60%,transparent_100%)]">
            <Badge variant="sky" className="mr-1.5 align-middle">
              {normalizedPreview}
            </Badge>
            <span className="ml-1.5">
              {t("settings.general.superWhisper.exampleText")}
            </span>
          </p>
        </div>

        {/* Tone: applied to every Hey Superflow output (email, Slack, prompt) */}
        <div className="mt-4 flex flex-col gap-2">
          <p className="text-sm font-medium tracking-tight text-stone-100">
            {t("settings.general.superWhisper.toneLabel")}
          </p>
          <div>
            <Dropdown
              options={TONE_OPTIONS}
              selectedValue={storedTone}
              onSelect={(value) => {
                void updateSetting("hey_superflow_tone", value as never);
              }}
              placeholder="Keep my voice"
              disabled={toneBusy}
              className="w-56"
            />
          </div>
        </div>

      </div>
    </SettingsGroup>
  );
});

VoiceHookCard.displayName = "VoiceHookCard";
