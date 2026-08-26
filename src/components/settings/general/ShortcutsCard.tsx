import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { Textarea } from "../../ui/Textarea";
import { Button } from "../../ui/Button";
import type { Shortcut } from "@/bindings";

const MAX_NAME = 40;
const MAX_CONTENT = 4000;

const sanitizeName = (name: string) =>
  name
    .replace(/[<>"'`]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_NAME);

interface Draft {
  id: string | null;
  name: string;
  content: string;
}

const EMPTY_DRAFT: Draft = { id: null, name: "", content: "" };

export const ShortcutsCard: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const shortcuts = useMemo(
    () => [...(getSetting("shortcuts") || [])].sort((a, b) => a.name.localeCompare(b.name)),
    [getSetting],
  );
  const [draft, setDraft] = useState<Draft | null>(null);

  const busy = isUpdating("shortcuts");

  const openAdd = () => setDraft({ ...EMPTY_DRAFT });
  const openEdit = (s: Shortcut) => setDraft({ id: s.id, name: s.name, content: s.content });

  const commit = () => {
    if (!draft) return;
    const name = sanitizeName(draft.name);
    const content = draft.content.trim().slice(0, MAX_CONTENT);
    if (!name || !content) {
      toast.error(t("settings.general.shortcuts.required"));
      return;
    }
    if (
      shortcuts.some(
        (s) => s.name.toLowerCase() === name.toLowerCase() && s.id !== draft.id,
      )
    ) {
      toast.error(t("settings.general.shortcuts.duplicateName"));
      return;
    }
    const next =
      draft.id === null
        ? [...shortcuts, { id: `sc_${Date.now()}`, name, content }]
        : shortcuts.map((s) => (s.id === draft.id ? { ...s, name, content } : s));
    updateSetting("shortcuts", next);
    setDraft(null);
  };

  const remove = (id: string) => {
    updateSetting(
      "shortcuts",
      shortcuts.filter((s) => s.id !== id),
    );
  };

  return (
    <div className="space-y-2">
      <div className="px-4">
        <h2 className="text-xs font-medium uppercase tracking-wide text-stone-500">
          {t("settings.general.shortcuts.title")}
        </h2>
        <p className="mt-1 text-xs leading-5 text-stone-500">
          {t("settings.general.shortcuts.description")}
        </p>
      </div>

      <div className="rounded-[10px] bg-surface">
        {/* List */}
        {shortcuts.length > 0 && (
          <div className="divide-y divide-stone-800">
            {shortcuts.map((s) => (
              <div key={s.id} className="group flex items-start gap-3 px-4 py-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium tracking-tight text-stone-100">
                    {s.name}
                  </p>
                  <p className="mt-0.5 truncate text-xs leading-5 text-stone-500">
                    {s.content.split("\n")[0]}
                  </p>
                </div>
                {!draft || draft.id !== s.id ? (
                  <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100">
                    <button
                      type="button"
                      onClick={() => openEdit(s)}
                      disabled={busy}
                      className="cursor-pointer rounded-md p-1.5 text-stone-500 transition-colors hover:bg-stone-800 hover:text-stone-100 disabled:cursor-not-allowed disabled:opacity-50"
                      aria-label={t("settings.general.shortcuts.edit")}
                    >
                      <svg className="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                      </svg>
                    </button>
                    <button
                      type="button"
                      onClick={() => remove(s.id)}
                      disabled={busy}
                      className="cursor-pointer rounded-md p-1.5 text-stone-500 transition-colors hover:bg-stone-800 hover:text-rose-400 disabled:cursor-not-allowed disabled:opacity-50"
                      aria-label={t("settings.general.shortcuts.delete")}
                    >
                      <svg className="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                    </button>
                  </div>
                ) : null}
              </div>
            ))}
          </div>
        )}

        {/* Add / Edit form */}
        <div className="p-4">
          {draft ? (
            <div className="space-y-3">
              <Input
                type="text"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                placeholder={t("settings.general.shortcuts.namePlaceholder")}
                maxLength={MAX_NAME}
                autoFocus
                className="w-full"
                disabled={busy}
              />
              <Textarea
                value={draft.content}
                onChange={(e) => setDraft({ ...draft, content: e.target.value })}
                placeholder={t("settings.general.shortcuts.contentPlaceholder")}
                rows={draft.id === null ? 3 : 5}
                maxLength={MAX_CONTENT}
                className="w-full resize-y font-mono text-xs leading-5"
                disabled={busy}
              />
              <div className="flex items-center justify-end gap-2">
                <Button variant="ghost" size="md" onClick={() => setDraft(null)} disabled={busy}>
                  {t("settings.general.shortcuts.cancel")}
                </Button>
                <Button
                  variant="primary"
                  size="md"
                  onClick={commit}
                  disabled={busy || !sanitizeName(draft.name) || !draft.content.trim()}
                >
                  {draft.id === null
                    ? t("settings.general.shortcuts.add")
                    : t("settings.general.shortcuts.save")}
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs leading-5 text-stone-500">
                {shortcuts.length === 0 && t("settings.general.shortcuts.empty")}
              </p>
              <Button variant="secondary" size="md" onClick={openAdd} disabled={busy}>
                <span className="mr-1.5 text-base leading-none">+</span>
                {t("settings.general.shortcuts.add")}
              </Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
