# Agent Design System

## Purpose

Build a coherent visual system, not isolated pretty screens.

The same system must remain consistent across:

* Landing pages
* Marketing pages
* Auth
* Onboarding
* Dashboards
* Settings
* Forms
* Tables
* Modals
* Components
* Empty states
* Product UI

The agent must not invent a new visual language for every page.

> **9/10 — keep 90%:** 5 questions only, brief/visual. Example answer: `Blue, Zinc, 6px, C (Stone 100 → White), Minimal` → build. No long theory before answers.

---

# 1. When to Ask Design Questions

Ask the design questions **only** when:

1. Creating a completely new design from scratch.
2. Redesigning an existing ugly/inconsistent design from scratch.

Do **not** ask them when extending an already-established system.

For an existing system:

> Inspect the current design system, identify its tokens/components, and continue it consistently.

---

# 2. CRITICAL: How Questions Must Be Asked

The agent must ask questions **briefly and visually**.

Do **not** produce a long explanation before asking.

Do **not** ask one question across hundreds of lines of Markdown.

Do **not** explain the entire design theory before the user has answered.

The initial interaction should look approximately like this:

```text
Before I build it, I need 5 design decisions:

1. Theme?
   Blue / Violet / Green / Orange / Pink / Yellow / Custom

2. Neutral?
   Slate / Zinc / Neutral / Stone / Custom

3. Roundedness?
   0px / 2–4px / 6–10px / Full

4. Surfaces?
   A. White page → Stone 50 cards
   B. White page → Stone 50 cards + subtle border
   C. Stone 100 page → White cards
   D. White page → White cards + subtle border

5. Badges?
   Minimal: bg-sky-500/15 text-sky-700
   Playful: bg-sky-600/5 border-sky-600/25 text-sky-700/80
```

That is the required level of questioning.

The agent should **not** turn those questions into a long questionnaire unless the user explicitly asks for detailed design exploration.

---

# 3. Theme

Ask:

> **Theme? Blue / Violet / Green / Orange / Pink / Yellow / Custom**

The theme is only the expressive accent.

Target approximately:

```text
90% neutral
10% theme
```

Use theme mainly for:

* Primary buttons
* Links
* Focus rings
* Input focus
* Selected states
* Active navigation
* Progress
* Small accents

Do not make the entire interface blue, purple, orange, etc.

---

# 4. Neutral Family

Ask:

> **Neutral? Slate / Zinc / Neutral / Stone / Custom**

The neutral family controls:

* Page backgrounds
* Cards
* Borders
* Inputs
* Dividers
* Muted text
* Hover states
* Secondary surfaces

### Custom Neutral

A custom base is allowed.

Example:

```text
#130F0C
```

If it is intentionally used as its own family, call it:

```text
dust
```

Do not pretend it is Tailwind Stone.

Example:

```text
dust-50
dust-100
dust-200
...
dust-950
```

The family name should describe the design system, not force a custom color into an existing palette.

---

# 5. Roundedness

Ask:

> **Roundedness? 0px / 2–4px / 6–10px / Full**

Choose one geometry language and keep it consistent.

Examples:

```text
0px
2px
4px
8px
9999px
```

Do not randomly mix:

```text
rounded-full
rounded-xl
rounded-lg
rounded-md
rounded-sm
```

without a system.

---

# 6. Surface / Card Treatment

Ask:

> **Surfaces?**
>
> A. `bg-white` page → `bg-stone-50` cards
> B. `bg-white` page → `bg-stone-50` cards + subtle border
> C. `bg-stone-100` page → `bg-white` cards
> D. `bg-white` page → `bg-white` cards + subtle border

The choice should establish the background hierarchy for the entire interface.

Do not invent unrelated card treatments later.

General hierarchy:

```text
Page
↓
Secondary surface
↓
Card
↓
Input
↓
Hover / active
```

Keep the number of layers low.

---

# 7. Badge Style

Ask:

> **Badges: minimal or playful?**

### Minimal

```tsx
"bg-sky-500/15 text-sky-700"
```

No border.

Example:

```tsx
type BadgeVariant =
  | "sky"
  | "violet"
  | "yellow"
  | "rose"
  | "green"
  | "orange";
```

### Playful

```tsx
"bg-sky-600/5 border border-sky-600/25 text-sky-700/80"
```

Example:

```tsx
const variantStyles: Record<BadgeVariant, string> = {
  sky: "bg-sky-600/5 border border-sky-600/25 text-sky-700/80",
  violet: "bg-violet-600/5 border border-violet-600/25 text-violet-700/80",
  yellow: "bg-yellow-600/5 border border-yellow-600/25 text-yellow-700/80",
  rose: "bg-rose-600/5 border border-rose-600/25 text-rose-700/80",
  green: "bg-green-600/5 border border-green-600/25 text-green-700/80",
  orange: "bg-orange-600/5 border border-orange-600/25 text-orange-700/80",
};
```

Badge geometry must inherit the global roundedness decision.

---

# 8. Typography

Default body typography:

```text
font-weight: 400
letter-spacing: normal
```

Do not globally apply:

```text
tracking-tight
tracking-[-0.02em]
tracking-[-0.025em]
```

unless there is a deliberate typographic reason.

Headings may use tighter tracking selectively.

Body text should remain natural and readable.

---

# 9. Buttons

Buttons use the established geometry and spacing system.

Prefer restrained controls rather than oversized generic SaaS buttons.

Example:

```tsx
className="
  inline-flex
  h-9
  items-center
  justify-center
  rounded-[4px]
  px-4
  text-sm
  font-medium
"
```

Actual radius follows the selected system.

---

# 10. Inputs and Focus

Inputs and textareas should use the neutral system by default.

Example:

```text
border-zinc-200
bg-white
text-zinc-900
```

Focus uses the theme:

```text
focus:border-sky-500
focus:ring-sky-500
```

Theme color should be especially visible in focus and active states.

---

# 11. Borders

Use subtle borders.

Typical hierarchy:

```text
Primary border
Secondary border
Muted divider
```

Do not add borders everywhere simply because an element exists.

If background contrast already separates a surface, a border may be unnecessary.

---

# 12. Shadows

Default:

```text
No shadow
```

Use shadows primarily for floating elements:

* Dropdowns
* Popovers
* Modals
* Menus
* Floating controls

Cards should normally rely on:

```text
background contrast
+
subtle border when necessary
```

not automatic shadows.

---

# 13. Hover

Hover states should come from the existing neutral/theme system.

Do not introduce unrelated colors.

Prefer simple transitions over:

* Glow
* Excessive blur
* Large scale changes
* Random gradients
* Gratuitous animation

---

# 14. New Landing Page

When creating from scratch, ask the compact five-question set:

```text
1. Theme?
   Blue / Violet / Green / Orange / Pink / Yellow / Custom

2. Neutral?
   Slate / Zinc / Neutral / Stone / Custom

3. Roundedness?
   0px / 2–4px / 6–10px / Full

4. Surfaces?
   White → Stone 50
   White → Stone 50 + border
   Stone 100 → White
   White → White + border

5. Badges?
   Minimal / Playful
```

Then build the system.

Do not keep asking design questions once these decisions are established.

---

# 15. Redesigning Existing UI

When replacing an ugly or inconsistent interface:

```text
Existing UI
↓
Keep content + functionality
↓
Discard inconsistent visual language
↓
Ask the same 5 design questions
↓
Establish the new system
↓
Rebuild consistently
```

The old styling is not authoritative.

---

# 16. Existing Product

Once the system exists:

**Stop asking the foundational questions.**

For new pages, screens, or components:

```text
Inspect current system
↓
Reuse existing tokens
↓
Reuse existing components
↓
Reuse existing variants
↓
Extend only when necessary
```

A new dashboard is not a new design system.

A new onboarding page is not a new design system.

A new settings page is not a new design system.

---

# 17. Consistency

The same design language must govern the whole product:

```text
Marketing
Pricing
Auth
Onboarding
Dashboard
Settings
Tables
Forms
Modals
Components
Empty states
Notifications
```

Do not create unrelated styling systems for different parts of the product unless that separation is explicitly intentional.

---

# 18. Component Rule

Reuse existing components.

Prefer:

```tsx
<Button variant="primary" />
<Button variant="secondary" />
<Button variant="ghost" />
```

over page-specific styling.

Likewise:

```text
Badge
Input
Select
Textarea
Card
Dialog
Tooltip
Tabs
Table
Dropdown
```

Once a component's visual language is established, reuse it.

---

# 19. Token Priority

When making a styling decision:

```text
Existing token
>
Existing component
>
Existing variant
>
New token
>
One-off styling
```

One-off styling is the last resort.

---

# 20. Anti-Pattern

Do not automatically generate generic SaaS styling:

```text
rounded-xl
rounded-full
shadow-md
gradient cards
glassmorphism
giant pills
huge buttons
blue gradients everywhere
```

"Modern SaaS" is not a design system.

---

# 21. Final Rule

The agent is building a **visual system**, not decorating individual pages.

Ask the essential questions once.

Make the decisions compactly.

Establish the tokens.

Build the interface.

Then enforce the same system everywhere.

**Consistency > novelty.**
**Intent > decoration.**
**System > one-off polish.**
