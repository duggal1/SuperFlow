import { useEffect, useState } from "react";
import { commands, type Theme } from "@/bindings";

/**
 * Appearance theme handling.
 *
 * SuperFlow already ships a full light palette and a full dark palette (see
 * `App.css`). This module lets the user pick which one is used instead of
 * always following the OS:
 *  - `system` removes the override so the `prefers-color-scheme` media query
 *    governs (the historical behaviour).
 *  - `light` / `dark` set `data-theme` on the document root, whose
 *    higher-specificity CSS selectors win over the media query.
 *
 * The choice is persisted in `AppSettings` (source of truth) and mirrored to
 * localStorage so it can be applied synchronously on boot, before React mounts,
 * avoiding a flash of the wrong palette.
 */

export const THEME_STORAGE_KEY = "superflow.theme";

export const THEME_OPTIONS: Theme[] = ["system", "light", "dark"];

const isTheme = (value: unknown): value is Theme =>
  value === "system" || value === "light" || value === "dark";

/** Apply a theme to the document root and remember it for the next launch. */
export const applyTheme = (theme: Theme): void => {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // localStorage may be unavailable (e.g. private mode); the setting still
    // persists in AppSettings, so this only costs a one-frame flash on boot.
  }
};

/** Read the last-applied theme for synchronous boot-time application. */
export const getStoredTheme = (): Theme => {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    if (isTheme(stored)) return stored;
  } catch {
    // ignore
  }
  return "system";
};

/** Apply the persisted theme from AppSettings (the source of truth). */
export const syncThemeFromSettings = async (): Promise<void> => {
  try {
    const result = await commands.getAppSettings();
    if (result.status === "ok") {
      applyTheme(result.data.theme ?? "system");
    }
  } catch (e) {
    console.warn("Failed to sync theme from settings:", e);
  }
};

/**
 * Resolve the *effective* appearance: explicit `light`/`dark` overrides win,
 * otherwise we follow the OS `prefers-color-scheme`. Re-renders whenever the
 * document `data-theme` or the OS color scheme changes.
 */
const computeIsLight = (): boolean => {
  const theme = document.documentElement.dataset.theme;
  if (theme === "light") return true;
  if (theme === "dark") return false;
  return window.matchMedia("(prefers-color-scheme: light)").matches;
};

export const useIsLight = (): boolean => {
  const [isLight, setIsLight] = useState<boolean>(() =>
    typeof document === "undefined" ? false : computeIsLight(),
  );

  useEffect(() => {
    const update = () => setIsLight(computeIsLight());
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    mq.addEventListener("change", update);
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => {
      mq.removeEventListener("change", update);
      observer.disconnect();
    };
  }, []);

  return isLight;
};
