# MISTAKE.md — Every Design Mistake Made (So No Agent Repeats It)

Read this + `DESIGN.md` before any frontend edit. These are real mistakes that made the owner pissed off, with why, what, and the fix.

## 1. Changed the hero description

- **What:** Changed `The most powerful way to do professional work with voice.` to `Connect your stack. Agents use it to act on your behalf — nothing extra to configure.` and later to `Gmail, Calendar, Drive and Docs — one tap to connect.` with `—` dashes.
- **Why it happened:** Tried to be "descriptive" and reworded hero copy without asking. Tone became robotic, bloated, dash-heavy (8+ em dashes across stack descriptions).
- **What was learned:** Never rewrite product copy. Header is sacred. 8–10 words, no `—`, humanized natural: `Connect Gmail, Calendar, Drive and Docs in one step` (9 words).
- **Now:** `AgentsPage.tsx:125` is locked to `The most powerful way to do professional work with voice.` Stack descs are fixed 8–9 words, no dashes.

## 2. Card: bloated bg, wrong stone, with border

- **What:** Cards rendered as `rounded-xl border border-stone-200/70 bg-white dark:bg-[#363230]` or `bg-[#35322f]` with `border-white/[0.06]`. Created 3 different card BGs across iterations (`#35322f`, `#363230`, `#373330`).
- **Why:** Invented stones instead of using the system. Added borders for "definition" that made cards look like outlined boxes, not editorial console.
- **What was learned:** Single card surface only: `rounded-xl bg-stone-800` **no border**. Expanded inner is `bg-[#32302d]` (stone 760). Never invent `#35322f` / `#363230` / `#373330` for cards.
- **Now:** `AgentsPage.tsx:138` `bg-stone-800`, expanded `bg-[#32302d]`. Checked via `DESIGN.md` tokens.

## 3. Button: blue/white mix, shadows, custom classes

- **What:** Used `inline-flex h-7 bg-stone-900 px-5 dark:bg-white` custom classes for Connect, and kept `shadow` for stone buttons. White mode had shadows on stone buttons; dark Disconnect used `border-stone-200 bg-white` custom.
- **Why:** Did not read `Button.tsx`. Invented button geometry per screen, leading to 4 different connect styles.
- **What was learned:** Every button must use `Button.tsx`. `Connect`/`Disconnect` on `bg-stone-800` = `variant="secondary" size="sm"` — dark `bg-[#363230] !shadow-none`, light `bg-white !shadow-none`, `px-4.5` (sm). Primary blue keeps `shadow-[0_0_0_1px_#2563eb26,inset…]` only. Never custom `h-7 bg-stone-900` strings.
- **Now:** `AgentsPage.tsx:180` all Connect/Disconnect are `<Button variant="secondary" size="sm">`.

## 4. Badge: dash instead of rose, wrong variant

- **What:** Sub-rows showed `—` when not connected; main cards used `variant="neutral"` (grey) for not connected; sometimes rendered `—` instead of badge.
- **Why:** Thought dash was "cleaner" and neutral was less alarming. Owner explicitly wants rose for not connected — it's a call to action.
- **What was learned:** Not connected is always `<Badge variant="rose">Not connected</Badge>` — every card, including expanded sub-rows (`Gmail`, `Google Calendar` etc.). Connected is `variant="green"` / `Included` emerald text. Provider batches are `orange` (Google) / `blue` (Microsoft).
- **Now:** `AgentsPage.tsx:160` rose badge, expanded `Badge variant="rose"`.

## 5. Bloated BYO cards

- **What:** BYO cards had `Bring your own Google OAuth keys` + 3-line bloated intro + nested `rounded-lg border bg-white` inside `bg-stone-800` card (double bg, double padding `p-5` outer + `p-5` inner), plus `OAuthSetupForm` rendered its own `h2` + `p` + 3 long i18n steps. Total ~40 words + 2 borders per card.
- **Why:** Copied `OAuthSetupForm.tsx` verbatim without de-bloating. Did not count words, did not remove nested surface.
- **What was learned:** BYO outer card is `bg-stone-800 p-5` with 11-word desc `Connect to your own Google app. Create your own OAuth keys — quick and private.` (`text-xs text-stone-400`). Inner `OAuthSetupForm` must be `bg-transparent` `space-y-3` (no `rounded-lg bg-stone-800 p-5`), bullets `text-stone-200` (tone 200), 3 short lines: `Create Google Cloud OAuth app` / `Add redirect URI from docs` / `Paste Client ID and Secret below`. Same for Microsoft. Total outer + inner = ~25 words, not 40.
- **Now:** `AgentsPage.tsx:230` BYO 11-word p + `OAuthSetupForm.tsx:42` `sectionClass="space-y-3"` + `stepClass="text-stone-200"`.

## 6. Text tone - invented greys

- **What:** Descriptions used `text-stone-500` (dark) / `text-stone-400` (light) — too muted; later changed to `text-stone-400` dark `text-stone-500` light without system; header subtitle flipped `text-stone-500` vs `text-stone-400`.
- **Why:** Guessed at "subtle" instead of following spec.
- **What was learned:** On `bg-stone-800` all body text is `text-stone-400` (header subtitle), card desc `text-stone-400` (12px), BYO desc `text-stone-400` (12px), bullets `text-stone-200` (12px). Light mode mirrors with `text-stone-500` where needed, but for agents page (dark-first) keep stone-400/200. Never `text-stone-500` on dark as primary desc.

## 7. Icons: phosphor with bg pill

- **What:** Permission cards used `Microphone size={32} text-stone-100` from `@phosphor-icons/react` inside `bg-surface` card, and success state used `bg-emerald-500/20 p-4` with `Check size={12} text-emerald-400` inside pill, plus `CircleNotch animate-spin` for waiting.
- **Why:** Copied phosphor defaults, added bg pills for "success" — owner explicitly hates `bg-green-100` pill with white icon.
- **What was learned:** Icons are raw `HugeiconsIcon` from `@hugeicons/core-free-icons` — `Mic02Icon size={28}`, `KeyboardIcon`, `CheckmarkCircle02Icon size={16} text-green-600 dark:text-green-500` *without* bg pill. Loading is `IOSSpinner size={12-14} color="currentColor"` from `global-spinner.tsx`. Keep button blue with spinner prefix.
- **Now:** `AccessibilityOnboarding.tsx:1` imports Hugeicons + IOSSpinner, renders `text-stone-700 dark:text-stone-100` raw.

## 8. Onboarding BG: neutral-900 instead of sidebar glass

- **What:** `AccessibilityOnboarding` used `bg-neutral-900` for checking / success / main screens; `App.tsx` used `bg-background` for onboarding wrapper; `Onboarding.tsx` had `p-6 gap-4 inset-0` with no surface.
- **Why:** Defaulted to Tailwind neutral, not the product's glass.
- **What was learned:** Onboarding BG must be `sidebar-material` — dark `rgba(20,20,21,0.18)` macOS native / `rgba(20,20,19,0.3) blur(32px) saturate(128%)` elsewhere, light mirrored `rgba(255,255,255,0.48/0.62)`. Applied via `App.tsx:433` `h-screen w-screen sidebar-material` for onboarding steps and `Onboarding.tsx:176` / `AccessibilityOnboarding.tsx:292` root `sidebar-material`.
- **Now:** All onboarding screens share sidebar glass, default theme dark.

## 9. Sidebar toggle: BG select, rounded 2, dim icon

- **What:** Toggle `rx="2"` outer / `rx="1"` inner, button `text-stone-500 hover:bg-stone-200` (light) `text-stone-400 hover:bg-white/[0.06]` (dark) with `bg-white/[0.04]` base. Looked like a select, not a ghost icon.
- **Why:** Copied `public/sidebar-fill.svg` defaults (rx 2 / 1.35) without tightening, left dim `400/500` and hover bg.
- **What was learned:** Toggle must be ghost: `rounded-[0.5px]` (`rx="0.5"` both rects), `text-stone-900` light / `text-stone-100` dark (brighter), `cursor-pointer` only, **no** `hover:bg-*`. `SidebarToggleIcon.tsx:29` `rx="0.5"`, `Sidebar.tsx:182` `text-stone-900 dark:text-stone-100` + `rounded-[0.5px]` no bg. `public/sidebar*.svg` also `rx="0.5"`.

## 10. Motion: cheap 0.22s snap + text stagger

- **What:** Sidebar `width 0↔176` with `duration: 0.22 ease [0.32,0.72,0,1]` and each section `motion.button opacity/x` with `delay idx*0.018` stagger. Felt cheap, text wobbled.
- **Why:** Used default snackable duration and added staggered text for "delight" without reading `skills/frontend/motion/*`.
- **What was learned:** Sidebar open/close is the only motion: `layout` + `width 0↔176` with `duration: 0.72 ease [0.16,1,0.3,1]` `willChange: transform` (transform, no reflow) — 120fps even with `Cmd+B` spam. Text has **zero** motion — plain `<button>`. Toggle has no bg. Reads `motion-react.md` `layout` (transform) + `scale` linear `linear(0,0.009…1.005)` for ultra-luxury, not `0.22s`.
- **Now:** `Sidebar.tsx:130` `duration: 0.72 ease [0.16,1,0.3,1] layout`, inner `nav` + buttons static.

## 11. Dropdown: wrong stone, heavy py

- **What:** `MenuPopup` `bg-stone-800 border-stone-700`, `MenuItem py-1`, `Dropdown trigger bg-stone-700 hover:bg-stone-600`, `Select menu var(--color-background)`.
- **Why:** Used generic `stone-800/700` without checking `DESIGN.md` tokens. Kept `py-1` (8px) when spec wants tighter.
- **What was learned:** Dark dropdowns are `bg-[#363230] border-white/[0.06]` (agents card is `bg-stone-800`, dropdown is `363230` — one step lighter), `py-0.5`, highlight `bg-white/[0.08]`, popup `p-0.5`, `Select` dark `bg-[#363230] border white/0.06` with `py-0.5` options. Light stays `bg-white border-stone-200/70`. Trigger (closed) is `bg-[#34302e]` (stone 850) `h-8 py-1`, open popup is `bg-[#363230]` wider `min-w-44`.
- **Now:** `Dropdown.tsx:62` `bg-[#363230] py-0.5`, `Select.tsx:42` `getSelectStyles(isLight)` dark `363230`.

## 12. Toast: custom div vs Sonner

- **What:** `CleanupModelToast.tsx` used `toast.custom(() => <div className="bg-stone-800 px-3 …"><CircleNotch …>…)` and `toast.success` with `CheckCircle`.
- **Why:** Invented toast UI per feature instead of using the system.
- **What was learned:** `src/components/toast.tsx` (`Sonner`, `SonnerState`, `IOSSpinner`, `HugeiconsIcon CircleCheck/AlertCircle`) is the **global default**. `App.tsx:3` renders `<Sonner sonner={sonner} />` (alongside legacy `Toaster` fallback). Every new feature `const [sonner,setSonner]=useState<SonnerState|null>(null)` + `<Sonner sonner={sonner} />` with `kind: "loading"|"success"|"error"|"warning"`.
- **Now:** `CleanupModelToast.tsx:1` imports `Sonner`, no `toast from "sonner"` custom div.

## 13. Meeting icon: sidebar vs dashboard

- **What:** Used `File02Icon` for meeting empty state center; sidebar `PeopleIcon` was correctly left but dashboard center was wrong file icon.
- **Why:** Reused `File02Icon` everywhere for "empty".
- **What was learned:** Dashboard center uses dedicated `MeetingIcon` (`src/components/icons/MeetingIcon.tsx` 18×18 calendar with dots) — `MeetingIcon className="size-8 text-stone-500"`. Sidebar stays `PeopleIcon`.
- **Now:** `MeetingPage.tsx:502` `MeetingIcon`.

## 14. Orb blank page (remote texture + R3F suspense, no boundary)

- **What:** Agents page rendered blank white. `orb.tsx` loaded `perlin-noise.png` from `storage.googleapis.com` via `useTexture`, which suspends; the only `Suspense fallback={null}` + `ErrorBoundary` returning `null` turned any texture/WebGL failure into a full blank. A `GenericOrb` isolation step proved the page itself was fine.
- **Why it happened (AI agent failure):** Reached for the heaviest component (WebGL + network texture) inside a page with no isolation, then wrapped the failure in `fallback={null}` + `return null` boundaries. Had to be supervised through `GenericOrb` → local asset → boundary iterations instead of isolating first.
- **What was learned:** `orb.tsx` stays untouched; it now bundles `perlin-noise.png` locally (`import.meta.url`) with `Suspense` inside `Canvas`. Every `Orb` mount sits inside `ErrorBoundary` (fallback message, never `null`) + `Suspense`. Never mount a suspending remote asset without a visible fallback.
- **Now:** `AgentsPage.tsx` wraps `TtsVoiceSection` in `ErrorBoundary context="TTS Voice"` + `Suspense`; `ErrorBoundary.tsx` renders a message and resets on `context` change.

## 15. Base-ui picker inside Tauri webview

- **What:** `voice-picker.tsx` used `@base-ui/react` `Popover` + `Command` (`@/components/ui/popover`, `@/components/ui/command`) for the voice dropdown — extra portal/focus machinery that blanked in the Tauri webview.
- **Why it happened (AI agent failure):** Copied a web-oriented ElevenLabs picker verbatim instead of building the native dropdown the design system already specifies (`Dropdown.tsx` tokens, absolute panel, outside-click close).
- **What was learned:** Agent picker surfaces use native React only: `useState` open/search, `useRef` outside-click, `Escape` close, `Button secondary` trigger, absolute panel. No `@base-ui/react` in `agent-picker/`.
- **Now:** `pocket-voice-picker.tsx` is fully native; `voice-picker.tsx` (ElevenLabs) is unused by `AgentsPage`.

## 16. White theme shipped as afterthought

- **What:** `DESIGN.md` specified dark tokens only; white mode was improvised per screen (`bg-white` page, invented borders, shadows on stone buttons), requiring repeated supervision passes (cards, View-all, CustomWords, toast, History button, About).
- **Why it happened (AI agent failure):** Treated white as a variant instead of a mirror, so every screen needed its own correction round instead of one spec read.
- **What was learned:** Every `DESIGN.md` section now carries Dark + White mirrors (`bg-stone-100` page, `bg-white 200/70` cards, `bg-stone-50` inners, `badgeVariantsLight`, secondary white buttons). Read the white mirror before touching any light class.
- **Now:** `DESIGN.md` sections 1–4 + 10 all specify both themes.

## Rule for Future Agents

Before touching `src/components/**`, `src/App.*`, `src/overlay/**`, or any `*.tsx` with Tailwind:

1. Read `src/components/DESIGN.md` — the only bg/card/badge/button/icon/toast/motion tokens.
2. Read `src/components/MISTAKE.md` — this file — and verify you are not repeating any of the 13 mistakes above.
3. Keep `bg-stone-900` page, `bg-stone-800` card, `bg-[#32302d]` expanded, `orange` Google / `blue` Microsoft, `rose` not connected, `Button.tsx secondary` stone, `VADIcon` duotone `24 1.65` with `stone-950 dark:white`, `sidebar-material` glass, `0.72s [0.16,1,0.3,1]` layout.
4. If you change copy, keep `The most powerful way to do professional work with voice.` and `Connect Gmail… in one step` (no dashes, 8–9 words).
