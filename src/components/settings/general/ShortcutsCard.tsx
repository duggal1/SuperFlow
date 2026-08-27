import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import ReactMarkdown from "react-markdown";
import { X } from "@phosphor-icons/react";
import { createPortal } from "react-dom";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { Textarea } from "../../ui/Textarea";
import { Button } from "../../ui/Button";
import { Badge } from "../../ui/Badge";
import type { Shortcut } from "@/bindings";

const MAX_NAME = 40;
const MAX_CONTENT = 4000;

const sanitizeName = (name: string) =>
  name
    .replace(/[<>"'`]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_NAME);

/** Ultra-clean markdown rendering for shortcut content. */
/** Linkify bare emails and http(s) URLs (skip ones already inside markdown links). */
const linkify = (text: string) =>
  text
    .replace(
      /([\w.+-]+@[\w-]+\.[\w.-]+[\w])/g,
      (m, _1, off, full: string) =>
        off > 0 && (full[off - 1] === "(" || full.slice(0, off).endsWith("]("))
          ? m
          : `[${m}](mailto:${m})`,
    )
    .replace(
      /(https?:\/\/[^\s<>()[\]]+)/g,
      (m, _1, off, full: string) =>
        off > 0 && (full[off - 1] === "(" || full.slice(0, off).endsWith("]("))
          ? m
          : `[${m}](${m})`,
    );

const Markdown = ({ children }: { children: string }) => (
  <div className="text-sm font-normal leading-6 tracking-tight text-stone-100 antialiased [&_a]:font-normal [&_a]:text-blue-600 [&_a]:underline [&_a]:decoration-blue-600 [&_a]:decoration-dotted [&_a]:underline-offset-2 hover:[&_a]:text-blue-500 [&_blockquote]:border-l-2 [&_blockquote]:border-stone-700 [&_blockquote]:pl-3 [&_blockquote]:text-stone-400 [&_code]:rounded-[3px] [&_code]:bg-stone-900 [&_code]:px-1 [&_code]:py-0.5 [&_code]:text-[13px] [&_h1]:mt-4 [&_h1]:text-lg [&_h1]:font-medium [&_h1]:tracking-tight [&_h1]:text-stone-50 [&_h1:first-child]:mt-0 [&_h2]:mt-4 [&_h2]:text-base [&_h2]:font-medium [&_h2]:tracking-tight [&_h2]:text-stone-100 [&_h2:first-child]:mt-0 [&_h3]:mt-3 [&_h3]:text-sm [&_h3]:font-medium [&_h3]:tracking-tight [&_h3]:text-stone-100 [&_hr]:border-stone-800 [&_ol]:mt-2 [&_ol]:list-decimal [&_ol]:space-y-1 [&_ol]:pl-5 [&_p]:my-2 [&_p:first-child]:mt-0 [&_p:last-child]:mb-0 [&_pre]:my-2 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-stone-900 [&_pre]:p-3 [&_strong]:font-medium [&_strong]:text-stone-50 [&_ul]:mt-2 [&_ul]:list-none [&_ul]:space-y-1 [&_ul>li]:relative [&_ul>li]:pl-4 [&_ul>li]:before:absolute [&_ul>li]:before:left-0 [&_ul>li]:before:top-[0.55em] [&_ul>li]:before:h-[5px] [&_ul>li]:before:w-[5px] [&_ul>li]:before:rounded-[1px] [&_ul>li]:before:bg-blue-600">
    <ReactMarkdown
      components={{
        a: (props) => <a {...props} target="_blank" rel="noreferrer" />,
        code: ({ children }) => {
          const value = String(children ?? "");
          if (
            /^\w[\w.-]*\.(tsx?|rs|jsx?|py|go|rb|java|swift|kt|css|scss|html|vue|svelte|prisma|sql|toml|ya?ml|json)$/.test(
              value,
            )
          ) {
            return <Badge variant="sky">{value}</Badge>;
          }
          return (
            <code className="rounded-[3px] bg-stone-900 px-1 py-0.5 text-[13px]">
              {children}
            </code>
          );
        },
      }}
    >
      {linkify(children)}
    </ReactMarkdown>
  </div>
);

/** Scrollable viewport with a bottom mask fade ONLY when content overflows. */
const OverflowFade = ({ children }: { children: React.ReactNode }) => {
  const ref = useRef<HTMLDivElement>(null);
  const [overflows, setOverflows] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const check = () =>
      setOverflows(el.scrollHeight > el.clientHeight + 2);
    check();
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return (
    <div className="relative min-h-0 w-full flex-1">
      <div
        ref={ref}
        className="max-h-[52vh] overflow-y-auto pr-1 [scrollbar-width:thin]"
      >
        {children}
      </div>
      {overflows && (
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-[#221f1d] via-[#221f1d]/70 to-transparent"
        />
      )}
    </div>
  );
};

interface Draft {
  id: string | null;
  name: string;
  content: string;
}

export const ShortcutsCard: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  // Direct read — store updates re-render immediately.
  const shortcuts = getSetting("shortcuts") || [];
  const busy = isUpdating("shortcuts");

  const [draft, setDraft] = useState<Draft | null>(null);
  const [viewing, setViewing] = useState<Shortcut | null>(null);
  const [editingView, setEditingView] = useState(false);

  const upsert = (next: Shortcut[]) => updateSetting("shortcuts", next);

  const commitDraft = () => {
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
    upsert(
      draft.id === null
        ? [...shortcuts, { id: `sc_${Date.now()}`, name, content }]
        : shortcuts.map((s) =>
            s.id === draft.id ? { ...s, name, content } : s,
          ),
    );
    setDraft(null);
  };

  const saveDialogEdits = () => {
    if (!viewing || !draft) return;
    const name = sanitizeName(draft.name);
    const content = draft.content.trim().slice(0, MAX_CONTENT);
    if (!name || !content) {
      toast.error(t("settings.general.shortcuts.required"));
      return;
    }
    if (
      shortcuts.some(
        (s) =>
          s.name.toLowerCase() === name.toLowerCase() && s.id !== viewing.id,
      )
    ) {
      toast.error(t("settings.general.shortcuts.duplicateName"));
      return;
    }
    const updated = { ...viewing, name, content };
    upsert(shortcuts.map((s) => (s.id === viewing.id ? updated : s)));
    setViewing(updated);
    setEditingView(false);
  };

  const removeById = (id: string) => {
    upsert(shortcuts.filter((s) => s.id !== id));
    if (viewing?.id === id) closeDialog();
  };

  const closeDialog = () => {
    setViewing(null);
    setEditingView(false);
    setDraft(null);
  };

  useEffect(() => {
    if (!viewing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeDialog();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [viewing]);

  const dialog =
    viewing !== null ? (
      <div
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) closeDialog();
        }}
      >
        <div className="relative flex max-h-[85vh] w-full max-w-2xl flex-col rounded-xl border-none bg-[#221f1d] p-5">
          {/* Close — top right, borderless */}
          <button
            type="button"
            onClick={closeDialog}
            aria-label={t("settings.general.shortcuts.cancel")}
            className="absolute right-3 top-3 cursor-pointer rounded-md p-2 text-stone-400 transition-colors duration-150 hover:bg-stone-700 hover:text-white"
          >
            <X className="h-4 w-4" weight="bold" />
          </button>

          {/* Title section — blue badge label */}
          <div className="flex flex-col items-start gap-2 pr-10">
            <Badge variant="neutral">{t("settings.general.shortcuts.titleField")}</Badge>
            {editingView && draft ? (
              <Input
                type="text"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                maxLength={MAX_NAME}
                autoFocus
                className="w-full"
                disabled={busy}
              />
            ) : (
              <p className="text-base font-medium tracking-tight text-stone-50">
                {viewing.name}
              </p>
            )}
          </div>

          {/* Replacement section — neutral badge label */}
          <div className="mt-4 flex min-h-0 flex-1 flex-col items-start gap-2">
            <Badge variant="blue">{t("settings.general.shortcuts.replacementField")}</Badge>
            {editingView && draft ? (
              <Textarea
                value={draft.content}
                onChange={(e) => setDraft({ ...draft, content: e.target.value })}
                rows={10}
                maxLength={MAX_CONTENT}
                className="w-full resize-y leading-6"
                disabled={busy}
              />
            ) : (
              <OverflowFade>
                <Markdown>{viewing.content}</Markdown>
              </OverflowFade>
            )}
          </div>

          {/* Footer — exact Button primitives, as before */}
          <div className="mt-4 flex shrink-0 items-center justify-end gap-2">
            {editingView ? (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setEditingView(false)}
                  disabled={busy}
                >
                  {t("settings.general.shortcuts.cancel")}
                </Button>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={saveDialogEdits}
                  disabled={
                    busy ||
                    !draft ||
                    draft.name.trim().length === 0 ||
                    draft.content.trim().length === 0
                  }
                >
                  {t("settings.general.shortcuts.save")}
                </Button>
              </>
            ) : (
              <>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => removeById(viewing.id)}
                  disabled={busy}
                  className="hover:!text-rose-400"
                >
                  {t("settings.general.shortcuts.delete")}
                </Button>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() => {
                    setDraft({
                      id: viewing.id,
                      name: viewing.name,
                      content: viewing.content,
                    });
                    setEditingView(true);
                  }}
                  disabled={busy}
                >
                  {t("settings.general.shortcuts.edit")}
                </Button>
              </>
            )}
          </div>
        </div>
      </div>
    ) : null;

  return (
    <div className="space-y-2">
      <div className="px-4">
        <h2 className="text-xs font-medium uppercase tracking-wide text-stone-500">
          {t("settings.general.shortcuts.title")}
        </h2>
      </div>

      <div className="rounded-[10px] bg-surface p-4">
        {/* Badge cloud — system Badge primitive */}
        <div className="flex flex-wrap items-center gap-2">
          {shortcuts.map((s) => (
            <span
              key={s.id}
              className="inline-flex items-stretch overflow-hidden rounded-[3.5px] bg-neutral-500/[0.11]"
            >
              <button
                type="button"
                onClick={() => {
                  setViewing(s);
                  setEditingView(false);
                  setDraft({ id: s.id, name: s.name, content: s.content });
                }}
                disabled={busy}
                className={`cursor-pointer px-3 py-1 text-[14px] font-medium leading-none tracking-[-0.09px] text-neutral-400 transition-colors hover:bg-neutral-500/[0.11] hover:text-neutral-300 disabled:cursor-not-allowed disabled:opacity-50`}
              >
                {s.name}
              </button>

            </span>
          ))}
        </div>

        {/* Add form — placeholders top, actions bottom right */}
        {draft && draft.id === null ? (
          <div className="mt-3 space-y-3">
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
              rows={3}
              maxLength={MAX_CONTENT}
              className="w-full resize-y leading-6"
              disabled={busy}
            />
            <div className="flex items-center justify-end gap-2">
              <Button
                variant="ghost"
                size="md"
                onClick={() => setDraft(null)}
                disabled={busy}
              >
                {t("settings.general.shortcuts.cancel")}
              </Button>
              <Button
                variant="primary"
                size="md"
                onClick={commitDraft}
                disabled={
                  busy ||
                  draft.name.trim().length === 0 ||
                  draft.content.trim().length === 0
                }
              >
                {t("settings.general.shortcuts.add")}
              </Button>
            </div>
          </div>
        ) : null}

        {/* Bottom row: hint left, Add Shortcut bottom right — always rendered */}
        <div className="mt-3 flex items-center justify-between gap-3">
          <p className="text-xs leading-5 text-stone-500">
            {shortcuts.length === 0 &&
              draft === null &&
              t("settings.general.shortcuts.empty")}
          </p>
          {!draft && (
            <Button variant="secondary" size="md" onClick={() => setDraft({ id: null, name: "", content: "" })} disabled={busy}>
              {t("settings.general.shortcuts.add")}
            </Button>
          )}
        </div>
      </div>

      {createPortal(dialog, document.body)}
    </div>
  );
});
