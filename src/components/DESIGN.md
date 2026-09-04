# DESIGN.md — Extreme UI Consistency

Read this before any frontend work. Also read `MISTAKE.md` + `src/components/agents/AGENTS.md`. `useIsLight()` drives every conditional. White theme is a first-class mirror in every section below, not an afterthought.

## 1. Page & Surface — Dark (Default) + White Mirror

- **Page BG — Dark (default):** `bg-stone-900` (`#1c1917`) — never `bg-background`, `bg-neutral-900`, or gradient. All full-screen onboarding / agents pages use `sidebar-material` or `bg-stone-900` directly. This is the editorial console base.
- **Page BG — White:** `bg-stone-100` (`#f5f5f4`) with `text-stone-900` title / `text-stone-500` desc. `AgentsPage.tsx:112` `isLight ? "bg-stone-100" : "bg-stone-900"` + `text-stone-900 : text-stone-100` header. Never `bg-white` for page, never `bg-stone-50` for page.
- **Card — Dark:** `rounded-xl bg-stone-800` — **no border** (`border-0`, not `border-white/[0.06]` / `border-stone-200/70`). `px-5 py-5` for stack cards, `p-5` for BYO. This is the only card surface. Do not invent `bg-[#35322f]` / `bg-[#363230]` / `bg-surface`.
- **Card — White:** `rounded-xl bg-white border border-stone-200/70` — same `px-5 py-5` / `p-5`, but white surface with `70%` stone-200 hairline. `AgentsPage.tsx:126` `isLight ? "border border-stone-200/70 bg-white" : "bg-stone-800"`. BYO cards same.
- **Expanded inner (View all) — Dark:** `rounded-lg bg-[#32302d]` — stone 760. This is the *only* nested surface inside a `bg-stone-800` card. Never `bg-[#373330]` / `bg-[#363230]` / `bg-stone-700`. `px-3 py-2.5`, `gap-2`.
- **Expanded inner — White:** `rounded-lg bg-stone-50` **no border** (`border-0`). `AgentsPage.tsx:195` `isLight ? "bg-stone-50" : "bg-[#32302d]"`. Never `bg-stone-100/80` with border.
- **Sub-row border:** none in both themes. Separation comes from bg step `white → stone-50` / `800 → #32302d`, not hairlines.
- **Grid:** `grid grid-cols-1 lg:grid-cols-2 gap-3` on `max-w-5xl`. Fullscreen = 2 cols. Never single column on `lg`, never `gap-4`+.
- **Spacing:** `gap-4` inside cards, `gap-3` between cards, `pt-6 pb-16` on page. No extra `p-1.5` wrappers.
- **White mode overview:** Page `bg-stone-100` / Cards `bg-white 200/70` + inner `bg-stone-50` + `View all` text underline + badges `Badge.tsx` light variants + buttons `Button.tsx secondary` white. See sections 3–4 for badge/button white specs.

## 2. Typography — Editorial, Not Robotic (Dark + White)

- **Page title — Dark:** `text-[22px] font-normal tracking-tight text-stone-100` / **White:** `text-stone-900`
- **Card title — Dark:** `text-[15px] font-normal tracking-tight text-stone-100` / **White:** `text-stone-900`
- **Description — Dark:** humanized, 8–10 words, no `—` dashes. Example: `Connect Gmail, Calendar, Drive and Docs in one step` (9 words). `text-[12px] leading-4 text-stone-400` on `bg-stone-800`. **White:** same copy, `text-stone-500` on `bg-white`.
- **Header subtitle (Agents) — Dark:** `The most powerful way to do professional work with voice.` — `text-[13px] leading-5 text-stone-400` / **White:** `text-stone-500 dark:text-stone-400` via `isLight`. Never rewrite to `Connect your stack...`.
- **BYO titles — Dark:** `text-sm font-normal text-stone-100` / **White:** `text-stone-900`
- **BYO desc — Dark:** 10–15 words max: `Connect to your own Google app. Create your own OAuth keys — quick and private.` — `text-xs leading-4 text-stone-400` / **White:** `text-stone-500` inside `bg-white` card. Never bloated paragraphs.
- **Bullets (1,2,3) — Dark:** `text-[12px] leading-5 tracking-tight text-stone-200` on `bg-stone-800`. **White:** same `text-stone-700`? Actually `OAuthSetupForm` uses `text-stone-200` on dark `bg-stone-800` inner, and for white inner `bg-stone-50` the bullets stay `text-stone-700` via `isLight`? Keep `text-stone-200` dark, `text-stone-600` white for steps: `Create Google Cloud OAuth app` / `Add redirect URI from docs` / `Paste Client ID and Secret below`. No `text-stone-500/400` on dark bullets.

## 3. Badge.tsx — The Only Pill (Dark + White)

- **Import:** `import { Badge } from "@/components/ui/Badge"` — has `badgeVariants` dark + `badgeVariantsLight` (`500/15` + `600`). `useIsLight()` picks automatically.
- **Google batch — Dark:** `variant="orange"` `bg-orange-500/10 text-orange-600 dark:bg-orange-500/15 dark:text-orange-400` / **White:** `bg-orange-500/15 text-orange-600` with `size-1.5 bg-orange-500` dot.
- **Microsoft batch — Dark/White:** `variant="blue"` same with blue `500/15` + `600`.
- **Connected — Dark:** `variant="green"` with emerald dot / **White:** `bg-green-500/15 text-green-600` (light) vs `bg-[#22c55e]/[0.11] text-[#22c55e]` dark.
- **Not connected — Both:** `variant="rose"` — **never** `variant="neutral"` or `—` dash. Every card, including expanded sub-rows when disconnected, shows `<Badge variant="rose">Not connected</Badge>`. Light: `bg-rose-500/15 text-rose-600`, dark: `bg-[#f43f5e]/[0.11] text-[#f43f5e]`.
- **Sub-row connected:** plain `text-emerald-500 text-[11px]` `Included` dark / `text-emerald-600` light — no badge, keeps density low.
- **Do not invent** `bg-green-100 text-green-700` pills or custom dot colors. Use `Badge.tsx` only. White mode already supported via `badgeVariantsLight`.

## 4. Button.tsx — The Only Button (Dark + White)

- **Import:** `import { Button } from "@/components/ui/Button"` — has `primaryShadow` only for `primary`; `secondary`/`ghost` are `!shadow-none`.
- **Connect / Disconnect — Dark:** `variant="secondary" size="sm"` on `bg-stone-800` renders `border-stone-700 bg-[#363230] hover:bg-[#363230] !shadow-none` `px-4.5` (sm) `text-stone-50` — stone, no blue. **White:** same `variant="secondary"` renders `border-stone-200/70 bg-white hover:bg-white !shadow-none` `px-4.5` `text-stone-900` with `hover:border-stone-200/90`. Primary blue (`variant="primary"`) keeps `shadow-[0_0_0_1px_#2563eb26,inset_0_2px_#ffffff30…]` in both themes — do not remove blue shadow.
- **History OpenRecordingsButton — Dark:** `variant="secondary"` `bg-stone-800` / **White:** `bg-white border-stone-200/70 !shadow-none px-4.5` (was `primary` blue).
- **CustomWords Add — Dark:** `variant="secondary"` `bg-stone-800 !border-0` / **White:** `!bg-white !border !border-stone-200/70 hover:!bg-stone-100/80 hover:!border-0` with `IOSSpinner`. Input white: `!bg-stone-100/80 !border-0` vs dark `!bg-stone-800 !border-stone-700`.
- **Word badge (custom words) — Dark:** `bg-stone-800 text-stone-100` / **White:** `bg-stone-100/80 text-stone-700 border-0 px-4 py-1.5` left-aligned `flex-wrap justify-start px-4 pt-3 pb-3`.
- **View all — Both:** plain `<button>` `text-xs underline-offset-4 hover:underline` `isLight ? text-stone-600 hover:text-stone-900 : text-stone-400 hover:text-stone-100` — **no** `bg`, `border`, `rounded-md`, `h-7`. Dark and white both text-only.
- **Permission grant (onboarding) — Both:** `variant="primary"` (blue) with `IOSSpinner size={12} color="white"` prefix when `isLoading`. Keep `primaryShadow` — do not remove blue shadow.
- **Cursor:** every interactive element `cursor-pointer`. Already in `Button.tsx`; for custom buttons add `cursor-pointer`.
- **Sizes:** `sm h-7 px-3.5/py-0.5`, `md h-7.5 px-4 / 4.5`, `lg h-10` — secondary in white gets `neutralSizes` (`px-4.5/5/6.5`) with `!shadow-none`.

## 5. Icons — Raw HugeIcons

- **Sidebar:** `src/components/icons/sidebar.tsx` — `HomeIcon`, `GeneralIcon`, `SparklesIcon`, `PeopleIcon`, `HistoryIcon`, `AboutIcon`, `VADIcon` — `viewBox 0 0 18 18` (VAD `24 24`), `fill="currentColor"` with `PRIMARY_OPACITY 0.94` / `SECONDARY_OPACITY 0.52`, `text-stone-950 dark:text-white` + `size-4 opacity-90`. Never use `@phosphor-icons/react` for sidebar.
- **Permission cards / empty states:** raw `HugeiconsIcon` — `Mic02Icon size={28} text-stone-700 dark:text-stone-100`, `KeyboardIcon`, `CheckmarkCircle02Icon size={16} text-green-600 dark:text-green-500` for granted. Never `bg-green-100` pill with white icon.
- **Loading:** `IOSSpinner` from `src/components/shared/global-spinner.tsx` — `size 12-14`, `color="currentColor"` or `white` for blue buttons. Never `CircleNotch animate-spin` from phosphor.

## 6. Toast — Sonner Default

- **Import:** `import { Sonner, type SonnerState } from "@/components/toast"`
- **`Sonner` is the global default.** `App.tsx` renders `<Sonner sonner={sonner} />` alongside legacy `Toaster` fallback. Every new feature must `const [sonner,setSonner]=useState<SonnerState|null>(null)` and `<Sonner sonner={sonner} />` with `kind: "loading"|"success"|"error"|"warning"`.
- **`CleanupModelToast.tsx`** is the reference: `loading` shows `Downloading… 42% · 1.2 MB/s`, `success` auto-hides after 3s, no custom `<div className="bg-stone-800 …">` via `toast.custom`.
- **Do not** use `toast from "sonner"` directly for new code.

## 7. Onboarding — Same Glass as Sidebar Dark

- **BG:** `sidebar-material` — dark: `rgba(20,20,21,0.18)` macOS native / `rgba(20,20,19,0.3) blur(32px) saturate(128%)` elsewhere. Light: `rgba(255,255,255,0.48/0.62)` same blur. Applied via `App.tsx` `h-screen w-screen sidebar-material` for onboarding steps and `Onboarding.tsx` / `AccessibilityOnboarding.tsx` root `sidebar-material`. Never `bg-neutral-900` / `bg-background`.
- **Permission cards:** `rounded-lg border border-stone-200/70 bg-white dark:border-white/[0.06] dark:bg-white/[0.04] p-4` with raw HugeIcons, not phosphor.

## 8. Motion — Sidebar Ultra Luxury (Read `skills/frontend/motion/*`)

- **Sidebar `motion.div layout`:** `width: 0↔176` with `duration: 0.72, ease: [0.16,1,0.3,1]`, `willChange: transform`, `layout` (transform, no reflow) → 120fps even with `Cmd+B` spam. Inner sections have **no** `motion.button` — text has zero animation; only the frame moves. Toggle button: **no** `hover:bg-*`, only `cursor-pointer` + `text-stone-100/900` + `rounded-[0.5px]` (`rx="0.5"` on both rects).

## 10. Voice selector (Pocket TTS) — Dark + White

- **Card:** same stack card (`rounded-xl px-5 py-5`, dark `bg-stone-800` no border / light `border border-stone-200/70 bg-white`). Header row `flex items-center justify-between` (badge vertically centered, never `items-start`).
- **Header left:** real `Orb` (`size-6` in `size-9 rounded-lg` tile, dark `bg-blue-600/20` / light `bg-blue-500/10`) + name `text-[15px] font-normal tracking-tight` only. No model description line — only the 200 MB line below.
- **200 MB line:** `One 200 MB download then instant voices on this device.` (10 words, no dashes) — `text-xs leading-4`, dark `text-stone-400` / light `text-stone-600`.
- **Picker:** custom `PocketVoicePicker` (native React, no base-ui). Trigger is `Button variant="secondary" size="sm"` full width (`!h-9 !justify-between !px-3`), leading orb `size-6`, name + trailing `ChevronsUpDown size-4 opacity-50`. Panel absolute `rounded-lg border`, dark `border-white/[0.06] bg-[#363230]` / light `border-stone-200/70 bg-white`, search input (dark `bg-[#363230]`) + `max-h-60` list. Rows: real `Orb size-8` (stable `seed` per voice) + name `text-sm font-medium truncate` + `Check size-4` when selected (`opacity-0` otherwise). Row hover/selected: dark `bg-stone-700` / light `bg-stone-100/80`.
- **Textarea:** dark `bg-[#363230]` / light `bg-white`, placeholder `Hey! I'm your voice. Just tell me what you're working on, and we will take it from there.` No first-audio/last-audio status text anywhere.
- **Preview:** no preview card, no container — bare `LiveWaveform` (`./waveform`, `barColor="#fafaf9"` stone-50, `height={44}`) mounted only when preview audio exists (`hasPreview`); `active` stays false (never requests mic), `processing={isPlaying || isSynthesizing}` animates while streaming/playing.
- **Spoken agent status:** every agent status string is spoken through the selected Pocket-TTS voice (`src/overlay/speak-status.ts`, same streaming scheduler as the preview). Automation per-step `taskStatus` (`automation-step` event) cancels the previous utterance; terminal `finalMessage` / `success_message` / failure and clarification strings speak once. New dictation stops speech. Generic AI pasted output is never spoken.
- **Voices:** pulled from backend `tts_voices` (`alba, jean, fantine, azelma`), selected persisted via `tts_selected_voice` / `tts_set_voice` (selection warms the voice so Play never pays conditioning). Synthesis reads stored voice; default `alba` keeps the legacy no-flag path.
- **No mock orb:** downloaded state has no decorative `size-20` orb block — orb lives in header + picker rows only.

## 9. Tokens — Do Not Invent

- **Stones:** `900 #1c1917`, `800 #292524` (card), `760 #32302d` (expanded), `700 #44403c`, `600 #57534e`, `500 #78716c`, `400 #a8a29e`, `300 #d6d3d1`, `200 #e7e5e4`. Never `#35322f`, `#363230` as card, `#373330` as expanded.
- **Radii:** `rounded-xl` cards, `rounded-lg` buttons/rows, `rounded-[0.5px]` toggle, `rounded-sm` badges. No `rounded-md` drift.
- **Borders:** cards have **no border**. Only expanded sub-rows / BYO inner use `bg-[#32302d]` step, not hairline.
