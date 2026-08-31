---
name: anti-slop
description: Prevent repeated AI-generated UI/UX failure patterns. Use when designing, redesigning, styling, reviewing, or polishing any frontend. This skill does not prescribe one aesthetic. It removes the recurring decisions that make interfaces feel generic, inconsistent, overbuilt, under-refined, or visually cheap.
user-invocable: true
---

# Anti-Slop Design Skill

You are not here to make a design "clean" by adding more design language.

You are here to remove the recurring decisions that make AI-generated interfaces ugly.

AI slop is not a color, framework, component library, or aesthetic. It is a pattern of weak judgment:
- designing components in isolation instead of as one system
- choosing defaults because they are available
- confusing more styling with more design
- confusing clean code with clean visuals
- adding UI instead of editing it
- repeating safe patterns until the page feels generated
- stopping when the implementation is technically complete instead of visually resolved

The goal is not minimalism. The goal is deliberate design.

A dense interface can be excellent. A maximalist site can be excellent. A brutalist page can be excellent. A glass interface can be excellent. The failure is when the choices are accidental, inconsistent, generic, or visually unresolved.

## 1. The Prime Directive: Judge the Render, Not the Code

Frontend code can look elegant while the rendered interface looks terrible.

A component may use:
- semantic tokens
- clean React abstractions
- perfect Tailwind classes
- a sensible grid
- modern animation primitives
- a component library

and still look visually cheap.

Never infer visual quality from implementation quality.

After meaningful UI work, inspect the rendered result as a composition. Judge:
- hierarchy
- proportion
- spacing
- density
- alignment
- color relationships
- typography
- contrast
- shape language
- repetition
- visual rhythm
- interaction states
- whether anything feels generic or accidental

If the code says "premium" but the pixels say "template," the pixels win.

## 2. Read the Existing Design Before Touching It

Most ugly AI redesigns begin when the agent starts styling before understanding the visual system.

For an existing product, inspect the relevant screens first. Do not redesign Home while ignoring Settings. Do not build a dialog without examining existing dialogs. Do not create a new card language because the current task happens to need a card.

Extract a short internal design contract:

### Atmosphere
What does the product feel like?
Examples: quiet, dense, technical, editorial, soft, industrial, playful, precise, cinematic.

### Palette
Identify:
- page/background surfaces
- elevated surfaces
- borders/dividers
- primary text
- secondary text
- accent/action color
- hover/selected states
- semantic states

Think in roles, not random color utilities.

### Typography
Identify:
- family
- normal body weight
- heading weight
- size hierarchy
- line-height character
- tracking character
- label treatment

### Geometry
Identify:
- radius family
- button shape
- input shape
- card/container shape
- icon container shape
- divider treatment

### Depth
Identify whether the product is:
- flat
- layered by surface color
- bordered
- softly shadowed
- glassy
- heavily elevated

Do not introduce a new depth model inside one component.

### Spacing
Identify:
- page gutters
- component padding
- control height
- vertical rhythm
- section gaps
- compact vs spacious areas

### Signature
Identify the few things that make this interface itself rather than a generic SaaS template.

Preserve those unless the user explicitly wants a new visual direction.

## 3. Consistency Is a System, Not Copy-Paste

Consistency does not mean every component looks identical.

It means components feel like members of the same family.

A new component should inherit the product's:
- palette logic
- type hierarchy
- radius logic
- border philosophy
- shadow philosophy
- spacing rhythm
- interaction behavior
- icon language

The most common AI failure is local optimization:
"This card looks good."
"This modal looks good."
"This settings row looks good."

Then the full product looks like three unrelated designers worked on it during different decades.

Before inventing a style, search for an existing equivalent. Extend the system before creating another one.

If Home, Settings, onboarding, sidebar, dialogs, and menus use different visual grammars without a deliberate reason, the design fails even if every screen looks acceptable alone.

## 4. Do Not Let Defaults Design the Product

Framework defaults are implementation conveniences, not creative direction.

Do not blindly inherit:
- default Tailwind palette combinations
- default shadcn styling
- default component-library radius
- generic white cards with gray borders
- automatic purple/blue gradients
- centered SaaS heroes
- three equal feature cards
- glassmorphism because blur exists
- giant headings because the viewport is wide
- pills because `rounded-full` looks "modern"

Use a default only when it genuinely matches the product.

If a component can be recognized as "default shadcn with different copy," it is unfinished.

## 5. Color: Build Relationships, Not a Bag of Grays

AI frequently produces color values that are individually reasonable and collectively awful.

Example failure:
- white page
- medium-gray card
- darker-gray hover
- near-black border
- unrelated cool-gray menu
- different neutral family in Settings
- accent used randomly

That is not a palette. It is a pile.

Use a coherent surface ladder.

For a restrained light interface, a valid family might behave like:
- page: soft neutral
- primary surface: white or near-white
- secondary surface: slightly tinted neutral
- border: barely stronger than the surrounding surface
- hover: one subtle step away from rest state
- text: strong neutral
- secondary text: muted neutral
- accent: one deliberate brand/action color

The exact values depend on the product. The relationship matters more than the literal colors.

### Color locks
- Do not mix warm and cool neutral families casually.
- Do not make hover dramatically darker than rest unless the interaction calls for it.
- Do not make borders the highest-contrast element in a quiet interface.
- Do not invent new accent colors for isolated components.
- Light and dark mode must preserve the same hierarchy, not merely invert colors.
- If the product already has semantic tokens, use them instead of scattering literal colors.

A clean interface usually separates surfaces through small differences, not violent contrast.

## 6. Borders: Stop Outlining Everything

AI uses borders as a substitute for hierarchy.

This produces:
- card inside card inside bordered panel
- bordered dropdown inside bordered toolbar
- input borders darker than the text hierarchy
- aggressive focus treatment everywhere
- boxes around content that already separates naturally

Before adding a border, ask:

**Does this element need a boundary to be understood?**

If surface contrast, whitespace, alignment, or grouping already communicates the boundary, the border may be unnecessary.

Default toward:
- no border, or
- a low-contrast hairline

Use strong borders intentionally for:
- selected states
- strong structural systems
- brutalist/industrial direction
- validation/error emphasis
- high-contrast design languages

Do not use harsh borders merely because the component feels "empty."

## 7. Shadows: Elevation Must Mean Something

Do not sprinkle shadows to make components look finished.

By default, prefer:
- flat surfaces
- subtle surface contrast
- whitespace
- hairline borders

Use a shadow when it communicates actual layering:
- modal over page
- floating command palette
- popover
- detached toolbar
- draggable/floating object

When used, keep it visually integrated with the background. Avoid heavy black drop shadows in otherwise soft interfaces.

If removing the shadow makes the hierarchy clearer, remove it.

## 8. Typography: Do Not Shout at the User

AI often creates "hierarchy" by making everything heavier.

Symptoms:
- semibold body copy
- bold settings labels
- bold buttons
- bold tab labels
- oversized headings
- secondary content nearly as strong as primary content

That creates visual shouting, not hierarchy.

For clean product UI:
- normal/regular is the default body voice
- medium is often enough for controls and important labels
- semibold/bold should be earned
- muted text should actually recede
- heading scale should reflect importance, not the agent's desire to make the screen look designed

Use weight, size, color, spacing, and placement together.

One aggressive variable is usually enough. Do not increase size, weight, contrast, and tracking simultaneously unless the art direction explicitly wants that intensity.

## 9. Shape Language: Radius Must Have Grammar

Random roundedness is one of the fastest ways to make a product feel stitched together.

Choose a radius grammar.

Example:
- small controls: 6-8px
- inputs/buttons: 8-10px
- containers/dialogs: 12-16px
- pills: only for things that are semantically pill-like

Or use a sharper system. Or a softer one.

The exact scale is contextual. The rule is coherence.

Avoid:
- one card at 8px, another at 24px, another full-pill
- pill buttons inside an otherwise square industrial layout
- heavily rounded dialogs with sharp inputs
- different radius values because individual components "look nicer" that way

Shape is a design language. Treat it like typography.

## 10. Spacing: Breathing Room Without Bloat

Clean does not mean huge padding.

AI has two spacing failures:
1. cramped everything
2. "premium" achieved by adding absurd empty space

Good spacing creates rhythm.

Use enough whitespace to:
- separate groups
- make hierarchy obvious
- prevent controls from feeling crowded
- let important content breathe

But do not let whitespace make the interface feel unfinished or disconnected.

### Control proportion
For buttons, tabs, pills, segmented controls, and compact interactive elements:
- horizontal padding should usually exceed vertical padding
- vertical padding should remain compact enough that the control feels intentional
- do not inflate `py` to manufacture importance

A useful default relationship is roughly:
- compact: Y 1 unit / X 2 units
- comfortable: Y 1 unit / X 2-3 units

This is a proportional principle, not a literal Tailwind mandate.

### Spacing consistency
Repeated component types should share:
- height
- padding
- icon gap
- label gap
- row rhythm

If five settings rows have five subtly different vertical rhythms, the interface feels wrong even when nobody can immediately explain why.

## 11. Less Is More Means Edit, Not "Make It Minimal"

AI slop is frequently content slop.

The agent fills empty space because it thinks every component needs:
- title
- subtitle
- description
- helper text
- badge
- metadata
- icon
- tooltip
- secondary action

It does not.

For every visible text element, ask:

**Would the interface become less understandable if this disappeared?**

If not, remove it.

Examples:
- If a dropdown item is obvious from its title, do not add a 20-word description.
- If a dialog title explains the task, do not repeat it in body copy.
- If a settings control is self-explanatory, do not add helper text.
- If the icon adds no recognition value, remove it.
- If a section headline says everything, do not add an eyebrow merely to make the section look designed.

Editing is part of design.

Do not use minimalism as an excuse for incompleteness. Remove noise, not necessary information.

## 12. Cards Are Not the Default Container

AI loves cards because cards solve layout without requiring composition.

That is precisely why they become slop.

Use a card when the content is genuinely:
- a separate object
- independently actionable
- elevated in hierarchy
- a reusable unit
- meaningfully grouped

Otherwise consider:
- whitespace
- aligned columns
- section grouping
- dividers
- surface changes
- direct placement on the page

If every piece of content lives in a rounded rectangle, nothing feels important.

Nested cards require exceptional justification.

## 13. Interaction States Must Feel Designed

A static screenshot is not the product.

Interactive elements need intentional:
- hover
- active
- focus-visible
- selected
- disabled
- loading
- empty
- error states

Common AI failures:
- clickable element without `cursor: pointer`
- hover state far harsher than rest state
- border suddenly becoming black on hover
- selected state using a different visual language
- focus ring that looks pasted on
- disabled state that becomes unreadable
- no feedback on click

Hover should normally be a small, legible state transition:
- slight surface shift
- subtle text/icon change
- restrained border change
- small movement when appropriate

Do not redesign the component on hover.

Focus-visible must remain accessible. "Clean" is not permission to remove keyboard affordance.

## 14. Repetition Is the Hidden AI Tell

Humans recognize generated design through repeated compositional habits.

Common tells:
- every section uses the same header structure
- every section starts with a tiny uppercase eyebrow
- every feature is a card
- every card has icon + title + description
- every panel uses the same border/radius regardless of function
- endless left-text/right-image alternation
- every action is a pill
- every hover uses the same lift-and-shadow effect
- every page is centered at the same max-width
- every surface uses the same padding
- every marketing page resembles Stripe/Linear regardless of brand

Consistency is good. Repetition without hierarchy is not.

Vary composition while preserving the system.

A design system should make the page coherent, not monotonous.

## 15. Generic "Clean SaaS" Is Not a Creative Direction

Stripe, Linear, Vercel, and similar references are useful when the brief actually calls for that language.

They are not universal answers to "make this good."

Do not automatically produce:
- dark hero
- gradient glow
- centered headline
- floating product screenshot
- three feature cards
- logo wall
- bento grid
- giant CTA
- footer

That is scaffolding, not art direction.

Before a greenfield design, identify:
- audience
- product category
- emotional goal
- brand character
- content type
- visual references
- acceptable level of experimentation

Then choose the design language.

## 16. Take One or Two Real Risks

Removing slop can produce another failure: perfectly clean, completely lifeless UI.

A strong design needs judgment, not only restraint.

For important pages, identify one or two places where the interface can become memorable.

Possible risk:
- unusual composition
- distinctive typography
- strong image crop
- asymmetry
- unexpected but coherent color relationship
- signature navigation behavior
- powerful negative space
- one exceptional motion sequence
- unique material treatment
- deliberate density shift

Do not make every element experimental.

The strongest work often has:
- a disciplined system
- mostly quiet execution
- one or two high-conviction decisions

Risk without discipline is noise.
Discipline without risk is template design.

## 17. Motion Is Not Decoration

Animation must communicate at least one of:
- hierarchy
- cause and effect
- state change
- spatial relationship
- narrative sequence
- feedback

If the only reason is "it looks cool," it is optional and usually removable.

Avoid:
- motion on every card
- infinite floating without meaning
- multiple animation libraries for ordinary UI
- giant scroll effects on utility screens
- hover physics that make controls harder to use
- animation that hides poor static composition

Build a strong static frame first.

If the page is ugly when paused, motion will usually make the ugliness move.

## 18. Landing Pages and Product UI Need Different Judgment

Do not force one design grammar onto every surface.

### Product UI
Prioritize:
- consistency
- density control
- interaction clarity
- state design
- information hierarchy
- speed
- predictable placement

Novelty should be concentrated in selective moments.

### Marketing / landing
Allow more:
- composition variance
- editorial typography
- imagery
- asymmetric rhythm
- motion
- narrative transitions
- expressive whitespace

### Dashboards
Do not turn every metric into a card.
Use alignment, grouping, typography, and sparse separators before adding containers.

### Settings
Settings should feel boring in the best way:
- clear
- compact
- predictable
- consistent
- low-noise

Do not "creative direct" a settings screen into a poster.

## 19. Light and Dark Mode Are the Same Design System

Do not design light mode, then mechanically invert it.

Both modes must preserve:
- hierarchy
- emphasis
- surface relationships
- interaction states
- brand character
- readability

Avoid mode-specific chaos where:
- light mode is soft and borderless
- dark mode suddenly has glowing borders
- cards gain random transparency
- accent saturation changes wildly
- radius or shadow behavior changes

Theme changes color relationships. It should not change the product's personality.

## 20. Effort: Do Not Stop at Scaffold Quality

AI agents frequently optimize for completion:
- create shell
- place components
- add basic Tailwind
- declare done

That produces functional mockups, not finished interfaces.

When visual quality matters, refinement is real work.

Inspect:
- exact component proportions
- line wrapping
- optical alignment
- text density
- icon size
- empty space
- awkward gaps
- hover transitions
- active states
- edge cases
- dark mode
- mobile
- visual relationship to neighboring screens

Do not add complexity merely to appear hardworking.

Spend effort on refinement, not architecture theater.

More frontend code is justified when it creates a visibly better experience. More abstractions are not.

## 21. The Anti-Slop Audit

Before shipping, inspect the rendered interface and answer these.

### System
- Does this screen clearly belong to the same product as adjacent screens?
- Did I introduce a new palette, radius, border, shadow, or typography language without a reason?
- Are repeated components actually consistent?

### Hierarchy
- Can the user understand what matters within a few seconds?
- Are too many elements competing at the same visual weight?
- Is typography doing hierarchy work, or merely getting larger and bolder?

### Color
- Is the neutral family coherent?
- Are hover/selected states only as strong as necessary?
- Are borders quieter than primary content?
- Is accent usage controlled?

### Spacing
- Is the interface breathing without feeling empty?
- Are controls too tall?
- Is horizontal padding proportionate to vertical padding?
- Are repeated rows aligned and rhythmically consistent?

### Content
- Can any title, description, label, badge, icon, or helper text disappear without losing meaning?
- Is the interface explaining obvious things?
- Did I add text because the layout felt empty?

### Containers
- Did I use cards because the content needs grouping, or because I did not design the layout?
- Are there cards inside cards?
- Could whitespace or a surface shift replace a border or shadow?

### Interaction
- Do all clickable elements feel clickable?
- Are hover/focus/active states coherent with rest state?
- Does any hover become visually harsher than the action deserves?

### Originality
- Does this look like the product, or like a generic AI-generated SaaS template?
- Did I repeat the same composition too many times?
- Is there at least one deliberate design decision that gives the work identity?
- Did experimentation serve the product instead of serving the designer's ego?

### Rendered reality
- Did I actually inspect the final UI?
- What looks wrong at first glance?
- What still feels generated?
- What would a strong human designer delete, soften, align, tighten, or rethink?

If a visible problem remains, the task is not finished.

## 22. Final Rule

Do not ask yourself:

> "Is this clean?"

Ask:

> "What is making this look cheap, generic, inconsistent, noisy, harsh, bloated, or unfinished?"

Then remove or resolve those causes one by one.

Clean design is usually not something you add.

It is what remains after weak decisions are gone.
