# React animation

Presented by+

Advertise in this space

[Motion for React](/) is a simple yet powerful animation library. Whether you're building hover effects, scroll-triggered animations, or complex animation sequences, this guide will provide an overview of all the ways you can animate in React with Motion.

## What you'll learn

- How to create your first animation with the `<motion.div />` component.
- Which values and elements you can animate.
- How to customise your animations with transition options.
- How to animate elements as they enter and exit the DOM.
- How to orchestrate animations with variants.

## Animate with <motion />

Most animations in Motion are created with the[](/docs/react-motion-component)`<motion />`[component](/docs/react-motion-component). Import it from `"motion/react"`:

```
import { motion } from "motion/react"
```

Every HTML & SVG element can be defined with a `motion` component:

```
<motion.div />
```

```
<motion.a href="#" />
```

```
<motion.circle cx={0} />
```

These work identically to their HTML/SVG counterparts - same props, same behaviour - but with additional animation props like `animate`, `whileHover`, and `exit`.

The most common animation prop is `animate`. When values passed to `animate` change, the element will automatically animate to that value.

```
<motion.div animate={{ opacity: 1 }} />
```

*Live example:* https://examples.motion.dev/react/state-updates?utm_source=embed

### Enter animations

We can set initial values for an element with the `initial` prop. So an element defined like this will fade in when it enters the DOM:

```
<motion.article
  initial={{ opacity: 0 }}
  animate={{ opacity: 1 }}
/>
```

## Animatable values

**Motion can animate any CSS value**, like `opacity`, `filter` etc.

```
<motion.section
  initial={{ filter: "blur(10px)" }}
  animate={{ filter: "none" }}
/>
```

It can even animate values that aren't normally animatable by browsers, like `background-image` or `mask-image`:

```
<motion.nav
  initial={{ maskImage: "linear-gradient(to right, rgba(0,0,0,1) 90%, rgba(0,0,0,0) 100%)" }}
  animate={{ maskImage: "linear-gradient(to right, rgba(0,0,0,1) 90%, rgba(0,0,0,1) 100%)" }}
/>
```

### Transforms

Unlike CSS, Motion can animate every transform axis independently.

```
<motion.div animate={{ x: 100 }} />
```

It supports the following special transform values:

- Translate: `x`, `y`, `z`
- Scale: `scale`, `scaleX`, `scaleY`
- Rotate: `rotate`, `rotateX`, `rotateY`, `rotateZ`
- Skew: `skewX`, `skewY`
- Perspective: `transformPerspective`

`motion` components also have enhanced `style` props, allowing you to use these shorthands statically:

```
<motion.section style={{ x: -20 }} />
```

Animating transforms independently provides great flexibility, especially when animating different transforms with gestures:

```
<motion.button
  initial={{ y: 10 }}
  animate={{ y: 0 }}
  whileHover={{ scale: 1.1 }}
  whileTap={{ scale: 0.9 }}
/>
```

```
<motion.li
  initial={{ transform: "translateX(-100px)" }}
  animate={{ transform: "translateX(0px)" }}
  transition={{ type: "spring" }}
/>
```

### Supported value types

Motion can animate any of the following value types:

- Numbers: `0`, `100` etc.
- Strings containing numbers: `"0vh"`, `"10px"` etc.
- Colors: All CSS color formats like hex, `rgba`, `hsla`, `oklch`, `oklab`, `color-mix` etc.
- Complex strings containing multiple numbers and/or colors (like `box-shadow`).
- `display: "none"/"block"` and `visibility: "hidden"/"visible"`.

### Value type conversion

In general, values can only be animated between two of the same type (i.e `"0px"` to `"100px"`).

Colors can be freely animated between hex, RGBA and HSLA types.

Additionally, `x`, `y`, `width`, `height`, `top`, `left`, `right` and `bottom` can animate between different value types.

```
<motion.div
  initial={{ x: "100%" }}
  animate={{ x: "calc(100vw - 50%)" }}
/>
```

It's also possible to animate `width` and `height` in to/out of `"auto"`.

```
<motion.div
  initial={{ height: 0 }}
  animate={{ height: "auto" }}
/>
```

### Transform origin

`transform-origin` has three shortcut values that can be set and animated individually:

- `originX`
- `originY`
- `originZ`

If set as numbers, `originX` and `Y` default to a progress value between `0` and `1`. `originZ` defaults to pixels.

```
<motion.div style={{ originX: 0.5 }} />
```

### CSS variables

Motion for React can animate CSS variables, and also use CSS variable definitions as animation targets.

#### Animating CSS variables

Sometimes it's convenient to be able to animate a CSS variable to animate many children:

```
<motion.ul
  initial={{ '--rotate': '0deg' }}
  animate={{ '--rotate': '360deg' }}
  transition={{ duration: 2, repeat: Infinity }}
>
  <li style={{ transform: 'rotate(var(--rotate))' }} />
  <li style={{ transform: 'rotate(var(--rotate))' }} />
  <li style={{ transform: 'rotate(var(--rotate))' }} />
</motion.ul>
```

### CSS variables as animation targets

HTML `motion` components accept animation targets with CSS variables:

```
<motion.li animate={{ backgroundColor: "var(--action-bg)" }} />
```

## Transitions

By default, Motion will create appropriate transitions for snappy animations based on the type of value being animated.

For instance, physical properties like `x` or `scale` are animated with spring physics, whereas values like `opacity` or `color` are animated with duration-based [easing curves](/docs/easing-functions).

However, you can define your own animations via [the](/docs/react-transitions)`transition`[prop](/docs/react-transitions).

```
<motion.div
  animate={{ x: 100 }}
  transition={{ ease: "easeOut", duration: 2 }}
/>
```

A default `transition` can be set for many components with the `MotionConfig`[component](/docs/react-motion-config):

```
<MotionConfig transition={{ duration: 0.3 }}>
  <motion.div animate={{ opacity: 1 }} />
  // etc
```

Or you can set a specific `transition` on any animation prop:

```
<motion.div
  animate={{ opacity: 1 }}
  whileHover={{
    opacity: 0.7,
    // Specific transitions override default transitions
    transition: { duration: 0.3 }
  }}
  transition={{ duration: 0.5 }}
/>
```

card.css/motion-appcard.cssCard.tsx1.card {2 transition: scale 200ms linear(3 0, 0.009, 0.036, 0.084, 0.157, 0.255, 0.378,4 0.522, 0.679, 0.832, 0.954, 1.029, 1.052, 1.038,5 1.011, 0.99, 0.984, 0.991, 1.001, 1.005, 16 );7}89.card:hover {10 scale: 1.2;11}MOTIONEaseSpringDuration0.3Delay0›Saved transitions12Visual editing for your agent.Edit and preview Motion and CSS transitions live in your code. Tune ease curves, springs, and durations without leaving your editor.Part of Motion AI Kit. One-time fee, lifetime access.

## Enter animations

When a `motion` component is first created, it'll automatically animate to the values in `animate` if they're different from those initially rendered, which you can either do via CSS or via [the](/docs/react-motion-value)`initial`[prop.](/docs/react-motion-value)

```
<motion.li
  initial={{ opacity: 0, scale: 0 }}
  animate={{ opacity: 1, scale: 1 }}
/>
```

*Live example:* https://examples.motion.dev/react/enter-animation?utm_source=embed

You can also disable the enter animation entirely by setting `initial={false}`. This will make the element render with the values defined in `animate`.

```
<motion.div initial={false} animate={{ y: 100 }} />
```

## Exit animations

Motion for React can animate elements as they're removed from the DOM.

In React, when a component is removed, it's usually removed instantly. Motion provides [the](/docs/react-animate-presence)`AnimatePresence`[component](/docs/react-animate-presence) which keeps elements in the DOM while they perform an animation defined with the `exit` prop.

```
<AnimatePresence>
  {isVisible && (
    <motion.div
      key="modal"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
    />
  )}
</AnimatePresence>
```

*Live example:* https://examples.motion.dev/react/exit-animation?utm_source=embed

## Keyframes

So far, we've set animation props like `animate` and `exit` to single values, like `opacity: 0`.

This is great when we want to animate from the current value to a new value. But sometimes we want to animate through a **series of values**. In animation terms, these are called **keyframes**.

All animation props can accept keyframe arrays:

```
<motion.div animate={{ x: [0, 100, 0] }} />
```

*Live example:* https://examples.motion.dev/react/keyframes?utm_source=embed

When we animate to an array of values, the element will animate through each of these values in sequence.

In the previous example, we explicitly set the initial value as `0`. But we can also say "use the current value" by setting the first value to `null`.

```
<motion.div animate={{ x: [null, 100, 0] }} />
```

*Live example:* https://examples.motion.dev/react/keyframes-wildcards?utm_source=embed

This way, if a keyframe animation is interrupting another animation, the transition will feel more natural.

### Wildcard keyframes

This `null` keyframe is called a **wildcard keyframe**. A wildcard keyframe simply takes the value before it (or the current value, if this is the first keyframe in the array).

Wildcard keyframes can be useful for holding a value mid-animation without having to repeat values.

```
<motion.div
  animate={{ x: [0, 100, null, 0 ] }}
  // same as x: [0, 100, 100, 0] but easier to maintain
/>
```

### Keyframe timing

By default, each keyframe is spaced evenly throughout the animation. You can override this by setting [the](/docs/react-transitions#times)`times`[option](/docs/react-transitions#times) via `transition`.

`times` is an array of progress values between `0` and `1`, defining where in the animation each keyframe should be positioned.

```
<motion.circle
  cx={500}
  animate={{
    cx: [null, 100, 200],
    transition: { duration: 3, times: [0, 0.2, 1] }
  }}
/>
```

`0` is the start of the animation, and `1` is the end of the animation. Therefore, `0.2` places this keyframe somewhere towards the start of the animation.

## Motion along a path

By default, `x` and `y` animate in a straight line. Pass `arc()` to `transition.path` to dynamically bend that line into a curve.

*Live example:* https://examples.motion.dev/react/add-to-basket?utm_source=embed

```
import { arc, motion } from "motion/react"

<motion.div
  animate={{ x: 200, y: -120 }}
  transition={{ duration: 0.6, path: arc() }}
/>
```

`arc()` works alongside any `x`/`y` animation, plus [layout animations](/docs/react-layout-animations). See the [`arc()` reference](/docs/arc) for more.

## Gesture animations

Motion for React has animation props that can define how an element animates when it [recognises a gesture](/docs/react-gestures).

Supported gestures are:

- `whileHover`
- `whileTap`
- `whileFocus`
- `whileDrag`
- `whileInView`

When a gesture starts, it animates to the values defined in `while-`, and then when the gesture ends it animates back to the values in `initial` or `animate`.

```
<motion.button
  initial={{ opacity: 0 }}
  whileHover={{ backgroundColor: "rgba(220, 220, 220, 1)" }}
  whileTap={{ backgroundColor: "rgba(255, 255, 255, 1)" }}
  whileInView={{ opacity: 1 }}
/>
```

*Live example:* https://examples.motion.dev/react/gestures?utm_source=embed

The [custom Cursor component](/docs/cursor) available in [Motion+](/plus) takes this a step further with magnetic and target-morphing effects as a user hovers clickable targets (like buttons and links):

```
<Cursor magnetic />
```

*Live example:* https://examples.motion.dev/react/ios-pointer?utm_source=embed

## Variants

The `animate` prop works well for single elements, but real interfaces often need coordinated animations across parent and child components. Variants solve this by defining named animation states that propagate through the component tree.

Variants are a set of named targets. These names can be anything.

```
const variants = {
  visible: { opacity: 1 },
  hidden: { opacity: 0 },
}
```

Variants are passed to `motion` components via the `variants` prop:

```
<motion.div variants={variants} />
```

These variants can now be referred to by a label, wherever you can define an animation target:

```
<motion.div
  variants={variants}
  initial="hidden"
  whileInView="visible"
  exit="hidden"
/>
```

You can also define multiple variants via an array:

```
animate={["visible", "danger"]}
```

*Live example:* https://examples.motion.dev/react/notifications-stack?utm_source=embed

### Propagation

Variants are useful for reusing and combining animation targets. But it becomes powerful for orchestrating animations throughout trees.

Variants will flow down through `motion` components. So in this example when the `ul` enters the viewport, all of its children with a "visible" variant will also animate in:

```
const list = {
  visible: { opacity: 1 },
  hidden: { opacity: 0 },
}

const item = {
  visible: { opacity: 1, x: 0 },
  hidden: { opacity: 0, x: -100 },
}

return (
  <motion.ul
    initial="hidden"
    whileInView="visible"
    variants={list}
  >
    <motion.li variants={item} />
    <motion.li variants={item} />
    <motion.li variants={item} />
  </motion.ul>
)
```

*Live example:* https://examples.motion.dev/react/variants?utm_source=embed

### Orchestration

By default, this children animations will start simultaneously with the parent. But with variants we gain access to new `transition` props `when`[and](/docs/react-transitions#orchestration)`delayChildren`.

```
const list = {
  visible: {
    opacity: 1,
    transition: {
      when: "beforeChildren",
      delayChildren: stagger(0.3), // Stagger children by .3 seconds
    },
  },
  hidden: {
    opacity: 0,
    transition: {
      when: "afterChildren",
    },
  },
}
```

### Dynamic variants

Each variant can be defined as a function that resolves when a variant is made active.

```
const variants = {
  hidden: { opacity: 0 },
  visible: (index) => ({
    opacity: 1,
    transition: { delay: index * 0.3 }
  })
}
```

These functions are provided a single argument, which is passed via the `custom` prop:

```
items.map((item, index) => <motion.div custom={index} variants={variants} />)
```

This way, variants can be resolved differently for each animating element.

## Animation controls

Declarative animations via `animate` and `whileHover` cover most UI interactions. For cases that need sequencing, timeline scrubbing, or triggering animations from events outside React's render cycle, the `useAnimate`[hook](/docs/react-use-animate) provides imperative controls:

- Animating any HTML/SVG element (not just `motion` components).
- Complex animation sequences.
- Controlling animations with `time`, `speed`, `play()`, `pause()` and other playback controls.

```
function MyComponent() {
  const [scope, animate] = useAnimate()

  useEffect(() => {
    const controls = animate([
      [scope.current, { x: "100%" }],
      ["li", { opacity: 1 }]
    ])

    controls.speed = 0.8

    return () => controls.stop()
  }, [])

  return (
    <ul ref={scope}>
      <li />
      <li />
      <li />
    </ul>
  )
}
```

## Animate content

By passing [a](/docs/react-motion-value)`MotionValue` as the child of a `motion` component, it will render its latest value in the HTML.

```
import { useMotionValue, motion, animate } from "motion/react"

function Counter() {
  const count = useMotionValue(0)

  useEffect(() => {
    const controls = animate(count, 100, { duration: 5 })
    return () => controls.stop()
  }, [])

  return <motion.pre>{count}</motion.pre>
}
```

This avoids React re-renders entirely. The `motion` component updates the DOM text node directly, making it suitable for high-frequency value changes like counters or live data.

*Live example:* https://examples.motion.dev/react/html-content?utm_source=embed

It's also possible to [animate numbers](/docs/react-animate-number) with a ticking counter effect using the `AnimateNumber` component in [Motion+](/plus) by passing them directly to the component:

```
<AnimateNumber>{value}</AnimateNumber>
```

*Live example:* https://examples.motion.dev/react/number-radix-slider?utm_source=embed

For text rather than numbers, the [`Typewriter` component](/docs/react-typewriter) types content out character by character, with the speed variance and backspacing of a real person. The [text animation guide](/docs/text-animation) maps every technique, from split text to scroll reveals.

## Next

In this guide we've covered the basic kinds of animations we can perform in Motion using its **animation props**. However, there's much more to discover.

Most of the examples on this page have used HTML elements, but Motion also has unique [SVG animation](/docs/react-svg-animation) features, like its simple line drawing API.

We've also only covered time-based animations, but Motion also provides powerful [scroll animation](/docs/react-scroll-animations) features like `useScroll` and `whileInView`.

It also provides a powerful [layout animation](/docs/react-layout-animations) engine, that can animate between any two layouts using performant transforms.

Finally, there's also a whole [Basics examples category](/examples?platform=react&category=basics) that covers all the basics of animating with Motion for React with live demos and copy-paste code.
# Layout animation

Presented by+

Advertise in this space

Motion (previously Framer Motion) can automatically animate an element's size and position whenever a layout change occurs - with a single prop. Add `layout` to animate a single component, or use `layoutId` to animate shared elements across components, creating seamless transitions between different UI states.

In this guide, we'll learn how to:

- **Animate layout changes** with a single prop.
- Create **shared element transitions** between components.
- Explore **advanced techniques**.
- **Troubleshoot** common layout animation issues.
- Understand the **differences** between Motion and the native View Transitions API.

## How to animate layout changes

To enable layout animations on a `motion` component, simply add the `layout` prop. Any layout change that happens as a result of a React render will now be automatically animated.

```
<motion.div layout />
```

Layout animation can animate previously unanimatable CSS values, like switching `justify-content` between `flex-start` and `flex-end`.

```
<motion.div
  layout
  style={{ justifyContent: isOn ? "flex-start" : "flex-end" }}
/>
```

*Live example:* https://examples.motion.dev/react/layout-animation?utm_source=embed

Or by using the `layoutId` prop, it's possible to match two elements and animate between them for some truly advanced animations.

```
<motion.li layoutId="item" />
```

It can handle anything from microinteractions to full page transitions.

*Live example:* https://examples.motion.dev/react/app-store?utm_source=embed

*Live example:* https://examples.motion.dev/react/reorder-items?utm_source=embed

Layout changes can be anything, changing `width`/`height`, number of grid columns, reordering a list, or adding/removing new items:

### Performance

Animating layout is traditionally slow, but Motion performs all layout animations using the CSS `transform` property for the highest possible performance.

### Shared layout animations

For more advanced shared layout animations, `layoutId` allows you to connect two different elements.

When a new component is added with a `layoutId` prop matching an existing component, it will automatically animate out from the old component.

```
isSelected && <motion.div layoutId="underline" />
```

*Live example:* https://examples.motion.dev/react/shared-layout-animation?utm_source=embed

If the original component is still on the page when the new one enters, they will automatically crossfade.

To animate an element back to its origin, you can use the `AnimatePresence` component to keep it in the DOM until its exit animation has finished.

```
<AnimatePresence>
  {isOpen && <motion.div layoutId="modal" />}
</AnimatePresence>
```

### Customise a layout animation

Layout animations can be customised using [the](/docs/react-transitions)`transition`[prop](/docs/react-transitions).

```
<motion.div layout transition={{ duration: 0.3 }} />
```

If you need to set a transition specifically for the layout animation while having a different transition for other properties (like `opacity`), you can define a dedicated `layout` transition.

```
<motion.div
  layout
  animate={{ opacity: 0.5 }}
  transition={{
    ease: "linear",
    layout: { duration: 0.3 }
  }}
/>
```

When performing a shared layout animation, the transition defined for element we're animating **to** will be used.

```
<>
  <motion.button
    layoutId="modal"
    onClick={() => setIsOpen(true)}
    // This transition will be used when the modal closes
    transition={{ type: "spring" }}
  >
    Open
  </motion.button>
  <AnimatePresence>
    {isOn && (
      <motion.dialog
        layoutId="modal"
        // This transition will be used when the modal opens
        transition={{ duration: 0.3 }}
      />
    )}
  </AnimatePresence>
</>
```

### Motion along a path

By default, layout animations move elements in a straight line from their old position to their new one. Pass `arc()` to `transition.layout.path` to curve that motion instead, including for shared `layoutId` transitions.

```
import { arc, motion } from "motion/react"

<motion.div layout transition={{ layout: { path: arc() } }} />
```

See the [`arc()` docs](/docs/arc) for the available options.

card.css/motion-appcard.cssCard.tsx1.card {2 transition: scale 200ms linear(3 0, 0.009, 0.036, 0.084, 0.157, 0.255, 0.378,4 0.522, 0.679, 0.832, 0.954, 1.029, 1.052, 1.038,5 1.011, 0.99, 0.984, 0.991, 1.001, 1.005, 16 );7}89.card:hover {10 scale: 1.2;11}MOTIONEaseSpringDuration0.3Delay0›Saved transitions12Visual editing for your agent.Edit and preview Motion and CSS transitions live in your code. Tune ease curves, springs, and durations without leaving your editor.Part of Motion AI Kit. One-time fee, lifetime access.

## Advanced use-cases

### Layout animations inside scrollable containers

To correctly animate layout within a scrollable container, you must add the `layoutScroll` prop to the scrollable element. This allows Motion to account for the element's scroll offset.

```
<motion.div layoutScroll style={{ overflow: "scroll" }} />
```

### Animating within fixed containers

To correctly animate layout within fixed elements, we need to provide them the `layoutRoot` prop.

```
<motion.div layoutRoot style={{ position: "fixed" }} />
```

This lets Motion account for the page's scroll offset when measuring children.

### Group layout animations

Layout animations are triggered when a component re-renders and its layout has changed.

```
function Accordion() {
  const [isOpen, setOpen] = useState(false)

  return (
    <motion.div
      layout
      style={{ height: isOpen ? "100px" : "500px" }}
      onClick={() => setOpen(!isOpen)}
    />
  )
}
```

But what happens when we have two or more components that don't re-render at the same time, but **do** affect each other's layout?

```
function List() {
  return (
    <>
      <Accordion />
      <Accordion />
    </>
  )
}
```

When one re-renders, for performance reasons the other won't be able to detect changes to its layout.

We can synchronise layout changes across multiple components by wrapping them in the `LayoutGroup component`.

```
import { LayoutGroup } from "motion/react"

function List() {
  return (
    <LayoutGroup>
      <Accordion />
      <Accordion />
    </LayoutGroup>
  )
}
```

When layout changes are detected in any grouped `motion` component, layout animations will trigger across all of them.

### Relative animation

Motion's layout animations use **parent-relative** calculations instead of **viewport or page-relative**.

What this means is if you have a parent and child performing a layout animation with different transitions, unlike the browser's View Transition API, the child will never get "left behind" by its parent.

By default, these calculations use the top left of the child, but you can change this with the `layoutAnchor` prop. This accepts `0`-`1` progress values for `x` and `y` where `0` is top/left and 1 is bottom/right.

```
// Pin element to center
<motion.ul layout>
  <motion.li
    layout
    layoutAnchor={{ x: 0.5, y: 0.5 }}
    transition={{ delay: 1 }}
  />
</motion.ul>
```

*Live example:* https://examples.motion.dev/react/layout-anchor?utm_source=embed

### Fixing child distortion during layout animations

Because `layout` animations use `transform: scale()`, they can sometimes visually distort children or certain CSS properties.

- **Child elements:** To fix distortion on direct children, these can also be given the `layout` prop.
- **Border radius and box shadow:**Motion automatically corrects distortion on these properties, but they must be set via the `style`, `animate` or other animation prop.

```
<motion.div layout style={{ borderRadius: 20 }} />
```

## Troubleshooting

### The component isn't animating

Ensure the component is **not** set to `display: inline`, as browsers don't apply `transform` to these elements.

Ensure the component is re-rendering when you expect the layout animation to start.

### Animations don't work during window resize

Layout animations are blocked during horizontal window resize to improve performance and to prevent unnecessary animations.

### SVG layout animations are broken

SVG components aren't currently supported with layout animations. SVGs don't have layout systems so it's recommended to directly animate their attributes like `cx` etc.

### Content is animating when the scrollbar appears

Layout changes can affect whether or not a scrollbar is visible. Scrollbars take up visible space, which means layouts are then subsequently affected by the scrollbar. Layout animations will apply to any layout change.

If you're finding that this is leading to unwanted layout animations, you can ensure the scrollbar space is reserved, even when no scrollbar is visible, with the `scrollbar-gutter` CSS rule.

```
body {
  overflow-y: auto;
  scrollbar-gutter: stable;
}
```

### The content stretches undesirably

This is a natural side-effect of animating `width` and `height` with `scale`.

Often, this can be fixed by providing these elements a `layout` animation and they'll be scale-corrected.

```
<motion.section layout>
  <motion.img layout />
</motion.section>
```

Some elements, like images or text that are changing between different aspect ratios, might be better animated with `layout="position"`.

### Border radius or box shadows are behaving strangely

Animating `scale` is performant but can distort some styles like `border-radius` and `box-shadow`.

Motion automatically corrects for scale distortion on these properties, but they must be set on the element via `style`.

```
<motion.div layout style={{ borderRadius: 20 }} />
```

### Border looks stretched during animation

Elements with a `border` may look stretched during the animation. This is for two reasons:

1. Because changing `border` triggers layout recalculations, it defeats the performance benefits of animating via `transform`. You might as well animate `width` and `height` classically.
2. `border` can't render smaller than `1px`, which limits the degree of scale correction that Motion can perform on this style.

A work around is to replace `border` with a parent element with padding that acts as a `border`.

```
<motion.div layout style={{ borderRadius: 10, padding: 5 }}>
  <motion.div layout style={{ borderRadius: 5 }} />
</motion.div>
```

## Technical reading

Interested in the technical details behind layout animations? Nanda does an incredible job of [explaining the challenges](https://www.nan.fyi/magic-motion) of animating layout with transforms using interactive examples. Matt, creator of Motion, did a [talk at Vercel conference](https://www.youtube.com/watch?v=5-JIu0u42Jc&ab_channel=Vercel) about the implementation details that is largely up to date.

## Motion's layout animations vs the View Transitions API

More browsers are starting to support the [View Transitions API](https://developer.mozilla.org/en-US/docs/Web/API/View_Transitions_API), which is similar to Motion's layout animations.

### Benefits of View Transitions API

The main two benefits of View Transitions is that **it's included in browsers** and **features a unique rendering system**.

#### Filesize

Because the View Transitions API is already included in browsers, it's cheap to implement very simple crossfade animations.

However, the CSS complexity can scale quite quickly. Motion's layout animations are around 12kb but from there it's very cheap to change transitions, add springs, mark matching

#### Rendering

Whereas Motion animates the elements as they exist on the page, View Transitions API does something quite unique in that it takes an image snapshot of the previous page state, and crossfades it with a live view of the new page state.

For shared elements, it does the same thing, taking little image snapshots and then crossfading those with a live view of the element's new state.

This can be leveraged to create interesting effects like full-screen wipes that aren't really in the scope of layout animations. [Framer's Page Effects](https://www.framer.com/academy/lessons/page-effects) were built with the View Transitions API and it also extensively uses layout animations. The right tool for the right job.

### Drawbacks to View Transitions API

There are quite a few drawbacks to the API vs layout animations:

- **Not interruptible**: Interrupting an animation mid-way will snap the animation to the end before starting the next one. This feels very janky.
- **Blocks interaction**: The animating elements overlay the "real" page underneath and block pointer events. Makes things feel quite sticky.
- **Difficult to manage IDs**: Layout animations allow more than one element with a `layoutId` whereas View Transitions will break if the previous element isn't removed.
- **Less performant:** View Transitions take an actual screenshot and animate via `width`/`height` vs layout animation's `transform`. This is measurably less performant when animating many elements.
- **Doesn't account for scroll**: If the page scroll changes during a view transition, elements will incorrectly animate this delta.
- **No relative animations:**If a nested element has a `delay` it will get "left behind" when its parent animates away, whereas Motion handles this kind of relative animation.
- **One animation at a time**: View Transitions animate the whole screen, which means combining it with other animations is difficult and other view animations impossible.

All-in-all, each system offers something different and each might be a better fit for your needs. In the future it might be that Motion also offers an API based on View Transitions API.

## FAQs

What is a layout animation?A layout animation automatically animates an element's size and position when the layout changes, like reordering a list, toggling an accordion, or switching grid columns. Instead of calculating start and end values yourself, add layout to a <motion /> component and Motion handles it automatically using transforms.How are layout animations performant if they animate size?Motion measures the layout change, then animates using CSS transform (translate + scale) instead of actually animating width and height. Animating transforms can entirely avoid triggering paint.Why does my content look stretched during a layout animation?When Motion uses scale to animate a size change, child elements can get visually distorted. Fix this by adding layout to the children too and Motion will calculate counter-scales them so they appear undistorted. For elements that change aspect ratio (like images), use layout="position" to only animate the position and let the size snap.What's the difference between Motion's layout animations and the View Transitions API?Both animate elements between layout states, but they work differently. Motion animates the actual elements using transforms: it's interruptible, doesn't block pointer events, and handles multiple simultaneous animations. View Transitions takes a screenshot of the old state and crossfades to the new one. It's built into browsers but can't be interrupted, blocks interaction during the transition, and is less performant when animating many elements.
# Transitions

Presented by+

Advertise in this space

A `transition` defines the type of animation used when animating between two values.

```
const transition = {
  duration: 0.8,
  delay: 0.5,
  ease: [0, 0.71, 0.2, 1.01],
}
```

```
// Motion component
<motion.div
  animate={{ x: 100 }}
  transition={transition}
/>

// animate() function
animate(".box", { x: 100 }, transition)
```

*Live example:* https://examples.motion.dev/react/transition?utm_source=embed

## Setting a transition

`transition` can be set on any animation prop, and that transition will be used when the animation fires.

```
<motion.div
  whileHover={{
    scale: 1.1,
    transition: { duration: 0.2 }
  }}
/>
```

### Value-specific transitions

When animating multiple values, each value can be animated with a different transition, with `default` handling all other values:

```
// Motion component
<motion.li
  animate={{
    x: 0,
    opacity: 1,
    transition: {
      default: { type: "spring" },
      opacity: { ease: "linear" }
    }
  }}
/>

// animate() function
animate("li", { x: 0, opacity: 1 }, {
  default: { type: "spring" },
  opacity: { ease: "linear" }
})
```

card.css/motion-appcard.cssCard.tsx1.card {2 transition: scale 200ms linear(3 0, 0.009, 0.036, 0.084, 0.157, 0.255, 0.378,4 0.522, 0.679, 0.832, 0.954, 1.029, 1.052, 1.038,5 1.011, 0.99, 0.984, 0.991, 1.001, 1.005, 16 );7}89.card:hover {10 scale: 1.2;11}MOTIONEaseSpringDuration0.3Delay0›Saved transitions12Visual editing for your agent.Edit and preview Motion and CSS transitions live in your code. Tune ease curves, springs, and durations without leaving your editor.Part of Motion AI Kit. One-time fee, lifetime access.

### Default transitions

It's possible to set default transitions via the `transition` prop. Either for specific `motion` components:

```
<motion.div
  animate={{ x: 100 }}
  transition={{ type: "spring", stiffness: 100 }}
/>
```

Or for a group of `motion` components [via](/docs/react-motion-config#transition)`MotionConfig`:

```
<MotionConfig transition={{ duration: 0.4, ease: "easeInOut" }}>
  <App />
</MotionConfig>
```

### Inheritance

By default, transitions of higher specificity will replace default transitions. For example:

```
<MotionConfig transition={{ duration: 1, ease: "linear" }}>
  <motion.div
    animate={{ x: 100 }}
    transition={{ ease: "easeInOut" }}
  />
</MotionConfig>
```

In this above example, `x` will animate with the default `duration` of `0.3`.

By setting `inherit: true`, a transition will inherit values from transitions with lower specificity.

```
<MotionConfig transition={{ duration: 1, ease: "linear" }}>
  <motion.div
    animate={{ x: 100 }}
    transition={{
      inherit: true, // duration 1 now inherited
      ease: "easeInOut"
    }}
  />
</MotionConfig>
```

This is also true of value-specific transitions:

```
<motion.div
  animate={{ x: 100, opacity: 1 }}
  transition={{
    duration: 1,
    ease: "easeInOut",
    opacity: {
      inherit: true, // inherit 1 second
      ease: "linear"
    }
  }}
/>
```

## Transition settings

#### type

**Default:**Dynamic

`type` decides the type of animation to use. It can be `"tween"`, `"spring"` or `"inertia"`.

[**Tween**](/docs/tween) animations are set with a duration and an easing curve.

**Spring** animations are either physics-based or duration-based.

Physics-based spring animations are set via `stiffness`, `damping` and `mass`, and these incorporate the velocity of any existing gestures or animations for natural feedback.

*Live example:* https://examples.motion.dev/react/app-store?utm_source=embed

Duration-based spring animations are set via a `duration` and `bounce`. These don't incorporate velocity but are easier to understand, and can also be [generated as pure CSS](/docs/css) for when you'd rather not ship Motion to the browser.

**Inertia** animations decelerate a value based on its initial velocity, usually used to implement inertial scrolling.

```
<motion.path
  animate={{ pathLength: 1 }}
  transition={{ duration: 2, type: "tween" }}
/>
```

#### Spring visualiser

### Tween

#### duration

**Default:**`0.3` (or `0.8` if multiple keyframes are defined)

The duration of the animation. Can also be used for `"spring"` animations when `bounce` is also set.

```
animate("ul > li", { opacity: 1 }, { duration: 1 })
```

#### ease

The easing function to use with tween animations. Accepts:

- Easing function name. E.g `"linear"`
- An array of four numbers to define a cubic bezier curve. E.g `[.17,.67,.83,.67]`
- A [JavaScript easing function](/docs/easing-functions), that accepts and returns a value `0`-`1`.

These are the available easing function names:

- `"linear"`
- `"easeIn"`, `"easeOut"`, `"easeInOut"`
- `"circIn"`, `"circOut"`, `"circInOut"`
- `"backIn"`, `"backOut"`, `"backInOut"`
- `"anticipate"`

When animating keyframes, `ease` can optionally be set as an array of easing functions to set different easings between each value:

```
<motion.div
  animate={{
    x: [0, 100, 0],
    transition: { ease: ["easeIn", "easeOut"] }
  }}
/>
```

For immediate visual feedback, you can edit CSS or Motion easing curves directly in your code editor with the [Motion AI Kit Extension](/docs/ai-kit-install).

>Motion+ · AI KitMake your AI agent a Motion expert.Give it current Motion context, MotionScore for Agents, and production-ready CSS spring generation.Part of Motion+. One-time fee, lifetime access.›/motion create a phot

#### times

When animating multiple keyframes, `times` can be used to adjust the position of each keyframe throughout the animation.

Each value in `times` is a value between `0` and `1`, representing the start and end of the animation.

```
<motion.div
  animate={{
    x: [0, 100, 0],
    transition: { times: [0, 0.3, 1] }
  }}
/>
```

There must be the same number of `times` as there are keyframes. Defaults to an array of evenly-spread durations.

### Spring

#### bounce

**Default:** `0.25`

`bounce` determines the "bounciness" of a spring animation.

`0` is no bounce, and `1` is extremely bouncy.

```
<motion.div
  animate={{ rotateX: 90 }}
  transition={{ type: "spring", bounce: 0.25 }}
/>
```

#### visualDuration

If `visualDuration` is set, this will override `duration`.

The visual duration is a time, **set in seconds**, that the animation will take to visually appear to reach its target.

In other words, the bulk of the transition will occur before this time, and the "bouncy bit" will mostly happen after.

This makes it easier to edit a spring, as well as visually coordinate it with other time-based animations.

```
<motion.div
  animate={{ rotateX: 90 }}
  transition={{
    type: "spring",
    visualDuration: 0.5,
    bounce: 0.25
  }}
/>
```

#### damping

**Default:** `10`

Strength of opposing force. If set to 0, spring will oscillate indefinitely.

```
<motion.a
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', damping: 300 }}
/>
```

#### mass

**Default:** `1`

Mass of the moving object. Higher values will result in more lethargic movement.

```
<motion.feTurbulence
  animate={{ baseFrequency: 0.5 }}
  transition={{ type: "spring", mass: 0.5 }}
/>
```

#### stiffness

**Default:** `1`

Stiffness of the spring. Higher values will create more sudden movement.

```
<motion.section
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', stiffness: 50 }}
/>
```

#### velocity

**Default:** Current value velocity

The initial velocity of the spring.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', velocity: 2 }}
/>
```

#### restSpeed

**Default:** `0.1`

End animation if absolute speed (in units per second) drops below this value and delta is smaller than `restDelta`.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', restSpeed: 0.5 }}
/>
```

#### restDelta

**Default:** `0.01`

End animation if distance is below this value and speed is below `restSpeed`. When animation ends, the spring will end.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', restDelta: 0.5 }}
/>
```

### Inertia

An animation that decelerates a value based on its initial velocity. Optionally, `min` and `max` boundaries can be defined, and inertia will snap to these with a spring animation.

This animation will automatically precalculate a target value, which can be modified with the `modifyTarget` property.

This allows you to add snap-to-grid or similar functionality.

Inertia is also the animation used for `dragTransition`, and can be configured via that prop.

#### power

**Default:**`0.8`

A higher power value equals a further calculated target.

```
<motion.div
  drag
  dragTransition={{ power: 0.2 }}
/>
```

#### timeConstant

**Default:**`700`

Adjusting the time constant will change the duration of the deceleration, thereby affecting its feel.

```
<motion.div
  drag
  dragTransition={{ timeConstant: 200 }}
/>
```

#### modifyTarget

A function that receives the automatically-calculated target and returns a new one. Useful for snapping the target to a grid.

```
<motion.div
  drag
  // dragTransition always type: inertia
  dragTransition={{
    power: 0,
    // Snap calculated target to nearest 50 pixels
    modifyTarget: target => Math.round(target / 50) * 50
  }}
/>
```

#### min

Minimum constraint. If set, the value will "bump" against this value (or immediately spring to it if the animation starts as less than this value).

```
<motion.div
  drag
  dragTransition={{ min: 0, max: 100 }}
/>
```

#### max

Maximum constraint. If set, the value will "bump" against this value (or immediately snap to it, if the initial animation value exceeds this value).

```
<motion.div
  drag
  dragTransition={{ min: 0, max: 100 }}
/>
```

#### bounceStiffness

**Default:**`500`

If `min` or `max` is set, this affects the stiffness of the bounce spring. Higher values will create more sudden movement.

```
<motion.div
  drag
  dragTransition={{
    min: 0,
    max: 100,
    bounceStiffness: 100
  }}
/>
```

#### bounceDamping

**Default:**`10`

If `min` or `max` is set, this affects the damping of the bounce spring. If set to `0`, spring will oscillate indefinitely.

```
<motion.div
  drag
  dragTransition={{
    min: 0,
    max: 100,
    bounceStiffness: 100
  }}
/>
```

### Orchestration

#### delay

**Default:**`0`

Delay the animation by this duration (in seconds).

```
animate(element, { filter: "blur(10px)" }, { delay: 0.3 })
```

By setting `delay` to a negative value, the animation will start that long into the animation. For instance to start 1 second in, `delay` can be set to -`1`.

#### repeat

**Default:**`0`

The number of times to repeat the transition. Set to `Infinity` for perpetual animation.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ repeat: Infinity, duration: 2 }}
/>
```

#### repeatType

**Default:**`"loop"`

How to repeat the animation. This can be either:

- `loop`: Repeats the animation from the start.
- `reverse`: Alternates between forward and backwards playback.
- `mirror`: Switches animation origin and target on each iteration.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{
    repeat: 1,
    repeatType: "reverse",
    duration: 2
  }}
/>
```

#### repeatDelay

**Default:**`0`

When repeating an animation, `repeatDelay` will set the duration of the time to wait, in seconds, between each repetition.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ repeat: Infinity, repeatDelay: 1 }}
/>
```

#### when

**Default:**`false`

With variants, describes when an animation should trigger, relative to that of its children.

- `"beforeChildren"`: Children animations will play after the parent animation finishes.
- `"afterChildren"`: Parent animations will play after the children animations finish.

```
const list = {
  hidden: {
    opacity: 0,
    transition: { when: "afterChildren" }
  }
}

const item = {
  hidden: {
    opacity: 0,
    transition: { duration: 2 }
  }
}

return (
  <motion.ul variants={list} animate="hidden">
    <motion.li variants={item} />
    <motion.li variants={item} />
  </motion.ul>
)
```

#### delayChildren

**Default:**`0`

With variants, setting `delayChildren` on a parent will delay child animations by this duration (in seconds).

```
const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: {
      delayChildren: 0.5
    }
  }
}

const item = {
  hidden: { opacity: 0 },
  show: { opacity: 1 }
}

return (
  <motion.ul
    variants={container}
    initial="hidden"
    animate="show"
  >
    <motion.li variants={item} />
    <motion.li variants={item} />
  </motion.ul>
)
```

Using the `stagger` function, we can stagger the delay across children.

```
const transition = {
  delayChildren: stagger(0.1)
}
```

By default, delay will stagger across children from first to last. By using `stagger`'s `from` option, we can stagger from the last child, the center, or a specific index.

```
const transition = {
  delayChildren: stagger(0.1, { from: "last" })
}
```
# Reduce bundle size

Presented by+

Advertise in this space

A great web experience doesn't just look and move beautifully, it should load quickly, too.

When measuring the gzipped and minified size of Motion for React using a bundle analysis website like [Bundlephobia](https://bundlephobia.com/package/framer-motion@7.2.0), you might see big numbers like **50kb** or more!

This is misleading. Motion for React exports many functions, most of which you won't import. JavaScript bundlers like [Rollup](https://rollupjs.org/) and [Webpack](https://webpack.js.org/) are capable of "tree shaking", which means that only the code you import is shipped to consumers.

You may only use a tiny, single hook from Motion for React, like `useReducedMotion`. So in that case the size would be closer to **1kb**.

However, Motion for React's primary animation APIs are `useAnimate` and `motion`. Most developers will choose to use at least one of these when using Motion, so let's see how to make them as small as possible.

## useAnimate

`useAnimate` is Motion for React's animation function, used for manually triggering and controlling animations.

It comes in two sizes, **mini** (2.3kb) and **hybrid** (17kb).

The mini version exclusively uses WAAPI for hardware accelerated animations, whereas the hybrid function can also animate sequences, motion values, independent transforms and a whole lot more.

At 2.3kb, `useAnimate` mini is the smallest animation library available for React.

## motion

The `motion`[component](/docs/react-motion-component) is Motion for React's most common animation API.

Because of its declarative, props-driven API, it's impossible for bundlers to tree shake it any smaller than **34kb**.

However, by using [the](/docs/react-lazy-motion)`m`[and](/docs/react-lazy-motion)`LazyMotion`[components](/docs/react-lazy-motion), you can bring this down significantly, to just under **4.6kb** for the initial render.

Then, with lazy-loading, you can defer the loading of animations and interactions until after your site has rendered.

### Reduce size

Instead of importing `motion`, import the slimmer `m` component.

```
import * as m from "motion/react-m"
```

`m` is used in the exact same way as `motion`, but unlike `motion`, the `m` component doesn't come preloaded with features like animations, [layout animations](/docs/react-layout-animations), or the drag gesture.

Instead, we load these in manually via the `LazyMotion` component. This lets you choose which features you load in, and whether you load them as part of the main bundle, or lazy load them.

```
import { LazyMotion, domAnimation } from "motion/react"

// Load only the domAnimation package
function App({ children }) {
  return (
    <LazyMotion features={domAnimation}>
      {children}
    </LazyMotion>
  )
}
```

### Available features

There are currently two **feature packages** you can load:

- `domAnimation`: This provides support for animations, variants, exit animations, and tap/hover/focus gestures. (**+15kb**)
- `domMax`: This provides support for all of the above, plus pan/drag gestures and layout animations. (**+25kb**)

In the future it might be possible to offer more granular feature packages, but for now these were chosen to reduce the amount of duplication between features, which could result in much more data being downloaded ultimately.

### Synchronous loading

By passing one of these feature packages to `LazyMotion`, they'll be bundled into your main JavaScript bundle.

```
import { LazyMotion, domAnimation } from "motion/react"

function App({ children }) {
  return (
    <LazyMotion features={domAnimation}>
      {children}
    </LazyMotion>
  )
}
```

### Lazy loading

If you're using a bundler like Webpack or Rollup, we can pass a dynamic import function to `features` that will fetch features only after we've performed our initial render.

First, create a file that exports only the features you want to load.

```
// features.js
import { domMax } from "motion/react"
export default domMax
```

Then, pass `features` a function that will dynamically load that file.

```
import { LazyMotion } from "motion/react"
import * as m from "motion/react-m"

// Make sure to return the specific export containing the feature bundle.
const loadFeatures = () =>
  import("./features.js").then(res => res.default)

// This animation will run when loadFeatures resolves.
function App() {
  return (
    <LazyMotion features={loadFeatures}>
      <m.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
      />
    </LazyMotion>
  )
}
```

### Strict mode

Because the normal `motion` component still preloads all of its functionality, including it anywhere will break the benefits of using `LazyMotion`.

To help prevent this, the `strict` prop can be set on `LazyMotion`. If a `motion` component is loaded anywhere within, it will throw with a reminder to render the `m` component instead.

```
function App() {
  // This will throw!
  return (
    <LazyMotion strict>
      <motion.div />
    </LazyMotion>
  )
}
```
# Transitions

Presented by+

Advertise in this space

A `transition` defines the type of animation used when animating between two values.

```
const transition = {
  duration: 0.8,
  delay: 0.5,
  ease: [0, 0.71, 0.2, 1.01],
}
```

```
// Motion component
<motion.div
  animate={{ x: 100 }}
  transition={transition}
/>

// animate() function
animate(".box", { x: 100 }, transition)
```

*Live example:* https://examples.motion.dev/react/transition?utm_source=embed

## Setting a transition

`transition` can be set on any animation prop, and that transition will be used when the animation fires.

```
<motion.div
  whileHover={{
    scale: 1.1,
    transition: { duration: 0.2 }
  }}
/>
```

### Value-specific transitions

When animating multiple values, each value can be animated with a different transition, with `default` handling all other values:

```
// Motion component
<motion.li
  animate={{
    x: 0,
    opacity: 1,
    transition: {
      default: { type: "spring" },
      opacity: { ease: "linear" }
    }
  }}
/>

// animate() function
animate("li", { x: 0, opacity: 1 }, {
  default: { type: "spring" },
  opacity: { ease: "linear" }
})
```

card.css/motion-appcard.cssCard.tsx1.card {2 transition: scale 200ms linear(3 0, 0.009, 0.036, 0.084, 0.157, 0.255, 0.378,4 0.522, 0.679, 0.832, 0.954, 1.029, 1.052, 1.038,5 1.011, 0.99, 0.984, 0.991, 1.001, 1.005, 16 );7}89.card:hover {10 scale: 1.2;11}MOTIONEaseSpringDuration0.3Delay0›Saved transitions12Visual editing for your agent.Edit and preview Motion and CSS transitions live in your code. Tune ease curves, springs, and durations without leaving your editor.Part of Motion AI Kit. One-time fee, lifetime access.

### Default transitions

It's possible to set default transitions via the `transition` prop. Either for specific `motion` components:

```
<motion.div
  animate={{ x: 100 }}
  transition={{ type: "spring", stiffness: 100 }}
/>
```

Or for a group of `motion` components [via](/docs/react-motion-config#transition)`MotionConfig`:

```
<MotionConfig transition={{ duration: 0.4, ease: "easeInOut" }}>
  <App />
</MotionConfig>
```

### Inheritance

By default, transitions of higher specificity will replace default transitions. For example:

```
<MotionConfig transition={{ duration: 1, ease: "linear" }}>
  <motion.div
    animate={{ x: 100 }}
    transition={{ ease: "easeInOut" }}
  />
</MotionConfig>
```

In this above example, `x` will animate with the default `duration` of `0.3`.

By setting `inherit: true`, a transition will inherit values from transitions with lower specificity.

```
<MotionConfig transition={{ duration: 1, ease: "linear" }}>
  <motion.div
    animate={{ x: 100 }}
    transition={{
      inherit: true, // duration 1 now inherited
      ease: "easeInOut"
    }}
  />
</MotionConfig>
```

This is also true of value-specific transitions:

```
<motion.div
  animate={{ x: 100, opacity: 1 }}
  transition={{
    duration: 1,
    ease: "easeInOut",
    opacity: {
      inherit: true, // inherit 1 second
      ease: "linear"
    }
  }}
/>
```

## Transition settings

#### type

**Default:**Dynamic

`type` decides the type of animation to use. It can be `"tween"`, `"spring"` or `"inertia"`.

[**Tween**](/docs/tween) animations are set with a duration and an easing curve.

**Spring** animations are either physics-based or duration-based.

Physics-based spring animations are set via `stiffness`, `damping` and `mass`, and these incorporate the velocity of any existing gestures or animations for natural feedback.

*Live example:* https://examples.motion.dev/react/app-store?utm_source=embed

Duration-based spring animations are set via a `duration` and `bounce`. These don't incorporate velocity but are easier to understand, and can also be [generated as pure CSS](/docs/css) for when you'd rather not ship Motion to the browser.

**Inertia** animations decelerate a value based on its initial velocity, usually used to implement inertial scrolling.

```
<motion.path
  animate={{ pathLength: 1 }}
  transition={{ duration: 2, type: "tween" }}
/>
```

#### Spring visualiser

### Tween

#### duration

**Default:**`0.3` (or `0.8` if multiple keyframes are defined)

The duration of the animation. Can also be used for `"spring"` animations when `bounce` is also set.

```
animate("ul > li", { opacity: 1 }, { duration: 1 })
```

#### ease

The easing function to use with tween animations. Accepts:

- Easing function name. E.g `"linear"`
- An array of four numbers to define a cubic bezier curve. E.g `[.17,.67,.83,.67]`
- A [JavaScript easing function](/docs/easing-functions), that accepts and returns a value `0`-`1`.

These are the available easing function names:

- `"linear"`
- `"easeIn"`, `"easeOut"`, `"easeInOut"`
- `"circIn"`, `"circOut"`, `"circInOut"`
- `"backIn"`, `"backOut"`, `"backInOut"`
- `"anticipate"`

When animating keyframes, `ease` can optionally be set as an array of easing functions to set different easings between each value:

```
<motion.div
  animate={{
    x: [0, 100, 0],
    transition: { ease: ["easeIn", "easeOut"] }
  }}
/>
```

For immediate visual feedback, you can edit CSS or Motion easing curves directly in your code editor with the [Motion AI Kit Extension](/docs/ai-kit-install).

>Motion+ · AI KitMake your AI agent a Motion expert.Give it current Motion context, MotionScore for Agents, and production-ready CSS spring generation.Part of Motion+. One-time fee, lifetime access.›/motion create a photos carousel on homepage- Searching Motion documentation for carousel- Searching Motion examples for “photo carousel”- Found 1 doc and 3 examples.Building new carousel using yourexisting design system.

#### times

When animating multiple keyframes, `times` can be used to adjust the position of each keyframe throughout the animation.

Each value in `times` is a value between `0` and `1`, representing the start and end of the animation.

```
<motion.div
  animate={{
    x: [0, 100, 0],
    transition: { times: [0, 0.3, 1] }
  }}
/>
```

There must be the same number of `times` as there are keyframes. Defaults to an array of evenly-spread durations.

### Spring

#### bounce

**Default:** `0.25`

`bounce` determines the "bounciness" of a spring animation.

`0` is no bounce, and `1` is extremely bouncy.

```
<motion.div
  animate={{ rotateX: 90 }}
  transition={{ type: "spring", bounce: 0.25 }}
/>
```

#### visualDuration

If `visualDuration` is set, this will override `duration`.

The visual duration is a time, **set in seconds**, that the animation will take to visually appear to reach its target.

In other words, the bulk of the transition will occur before this time, and the "bouncy bit" will mostly happen after.

This makes it easier to edit a spring, as well as visually coordinate it with other time-based animations.

```
<motion.div
  animate={{ rotateX: 90 }}
  transition={{
    type: "spring",
    visualDuration: 0.5,
    bounce: 0.25
  }}
/>
```

#### damping

**Default:** `10`

Strength of opposing force. If set to 0, spring will oscillate indefinitely.

```
<motion.a
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', damping: 300 }}
/>
```

#### mass

**Default:** `1`

Mass of the moving object. Higher values will result in more lethargic movement.

```
<motion.feTurbulence
  animate={{ baseFrequency: 0.5 }}
  transition={{ type: "spring", mass: 0.5 }}
/>
```

#### stiffness

**Default:** `1`

Stiffness of the spring. Higher values will create more sudden movement.

```
<motion.section
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', stiffness: 50 }}
/>
```

#### velocity

**Default:** Current value velocity

The initial velocity of the spring.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', velocity: 2 }}
/>
```

#### restSpeed

**Default:** `0.1`

End animation if absolute speed (in units per second) drops below this value and delta is smaller than `restDelta`.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', restSpeed: 0.5 }}
/>
```

#### restDelta

**Default:** `0.01`

End animation if distance is below this value and speed is below `restSpeed`. When animation ends, the spring will end.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ type: 'spring', restDelta: 0.5 }}
/>
```

### Inertia

An animation that decelerates a value based on its initial velocity. Optionally, `min` and `max` boundaries can be defined, and inertia will snap to these with a spring animation.

This animation will automatically precalculate a target value, which can be modified with the `modifyTarget` property.

This allows you to add snap-to-grid or similar functionality.

Inertia is also the animation used for `dragTransition`, and can be configured via that prop.

#### power

**Default:**`0.8`

A higher power value equals a further calculated target.

```
<motion.div
  drag
  dragTransition={{ power: 0.2 }}
/>
```

#### timeConstant

**Default:**`700`

Adjusting the time constant will change the duration of the deceleration, thereby affecting its feel.

```
<motion.div
  drag
  dragTransition={{ timeConstant: 200 }}
/>
```

#### modifyTarget

A function that receives the automatically-calculated target and returns a new one. Useful for snapping the target to a grid.

```
<motion.div
  drag
  // dragTransition always type: inertia
  dragTransition={{
    power: 0,
    // Snap calculated target to nearest 50 pixels
    modifyTarget: target => Math.round(target / 50) * 50
  }}
/>
```

#### min

Minimum constraint. If set, the value will "bump" against this value (or immediately spring to it if the animation starts as less than this value).

```
<motion.div
  drag
  dragTransition={{ min: 0, max: 100 }}
/>
```

#### max

Maximum constraint. If set, the value will "bump" against this value (or immediately snap to it, if the initial animation value exceeds this value).

```
<motion.div
  drag
  dragTransition={{ min: 0, max: 100 }}
/>
```

#### bounceStiffness

**Default:**`500`

If `min` or `max` is set, this affects the stiffness of the bounce spring. Higher values will create more sudden movement.

```
<motion.div
  drag
  dragTransition={{
    min: 0,
    max: 100,
    bounceStiffness: 100
  }}
/>
```

#### bounceDamping

**Default:**`10`

If `min` or `max` is set, this affects the damping of the bounce spring. If set to `0`, spring will oscillate indefinitely.

```
<motion.div
  drag
  dragTransition={{
    min: 0,
    max: 100,
    bounceStiffness: 100
  }}
/>
```

### Orchestration

#### delay

**Default:**`0`

Delay the animation by this duration (in seconds).

```
animate(element, { filter: "blur(10px)" }, { delay: 0.3 })
```

By setting `delay` to a negative value, the animation will start that long into the animation. For instance to start 1 second in, `delay` can be set to -`1`.

#### repeat

**Default:**`0`

The number of times to repeat the transition. Set to `Infinity` for perpetual animation.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ repeat: Infinity, duration: 2 }}
/>
```

#### repeatType

**Default:**`"loop"`

How to repeat the animation. This can be either:

- `loop`: Repeats the animation from the start.
- `reverse`: Alternates between forward and backwards playback.
- `mirror`: Switches animation origin and target on each iteration.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{
    repeat: 1,
    repeatType: "reverse",
    duration: 2
  }}
/>
```

#### repeatDelay

**Default:**`0`

When repeating an animation, `repeatDelay` will set the duration of the time to wait, in seconds, between each repetition.

```
<motion.div
  animate={{ rotate: 180 }}
  transition={{ repeat: Infinity, repeatDelay: 1 }}
/>
```

#### when

**Default:**`false`

With variants, describes when an animation should trigger, relative to that of its children.

- `"beforeChildren"`: Children animations will play after the parent animation finishes.
- `"afterChildren"`: Parent animations will play after the children animations finish.

```
const list = {
  hidden: {
    opacity: 0,
    transition: { when: "afterChildren" }
  }
}

const item = {
  hidden: {
    opacity: 0,
    transition: { duration: 2 }
  }
}

return (
  <motion.ul variants={list} animate="hidden">
    <motion.li variants={item} />
    <motion.li variants={item} />
  </motion.ul>
)
```

#### delayChildren

**Default:**`0`

With variants, setting `delayChildren` on a parent will delay child animations by this duration (in seconds).

```
const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: {
      delayChildren: 0.5
    }
  }
}

const item = {
  hidden: { opacity: 0 },
  show: { opacity: 1 }
}

return (
  <motion.ul
    variants={container}
    initial="hidden"
    animate="show"
  >
    <motion.li variants={item} />
    <motion.li variants={item} />
  </motion.ul>
)
```

Using the `stagger` function, we can stagger the delay across children.

```
const transition = {
  delayChildren: stagger(0.1)
}
```

By default, delay will stagger across children from first to last. By using `stagger`'s `from` option, we can stagger from the last child, the center, or a specific index.

```
const transition = {
  delayChildren: stagger(0.1, { from: "last" })
}
```
