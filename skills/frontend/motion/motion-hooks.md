# Motion values overview

Presented by+

Advertise in this space

Motion values track the state and velocity of animated values.

They are composable, signal-like values that are performant because Motion can render them with its optimised DOM renderer.

Usually, these are created automatically by `motion`[components](/docs/react-motion-component). But for advanced use cases, it's possible to create them manually.

```
import { motion, useMotionValue } from "motion/react"

export function MyComponent() {
  const x = useMotionValue(0)
  return <motion.div style={{ x }} />
}
```

By manually creating motion values you can:

- Set and get their state.
- Pass to multiple components to synchronise motion across them.
- Chain `MotionValue`s via the `useTransform` hook.
- Update visual properties without triggering React's render cycle.
- Subscribe to updates.

```
const x = useMotionValue(0)
const opacity = useTransform(
  x,
  [-200, 0, 200],
  [0, 1, 0]
)

// Will change opacity as element is dragged left/right
return <motion.div drag="x" style={{ x, opacity }} />
```

## Usage

Motion values can be created with the `useMotionValue` hook. The string or number passed to `useMotionValue` will act as its initial state.

```
import { useMotionValue } from "motion/react"

const x = useMotionValue(0)
```

Motion values can be passed to a `motion` component via `style`:

```
<motion.li style={{ x }} />
```

Or for SVG attributes, via the attribute prop itself:

```
<motion.circle cx={cx} />
```

It's possible to pass the same motion value to multiple components.

Motion values can be updated with the `set` method.

```
x.set(100)
```

Changes to the motion value will update the DOM **without triggering a React re-render**. Motion values can be updated multiple times but renders will be batched to the next animation frame.

A motion value can hold any string or number. We can read it with the `get` method.

```
x.get() // 100
```

Motion values containing a number can return a velocity via the `getVelocity` method. This returns the velocity as calculated **per second** to account for variations in frame rate across devices.

```
const xVelocity = x.getVelocity()
```

For strings and colors, `getVelocity` will always return `0`.

### Events

Listeners can be added to motion values via [the](/docs/react-motion-value#on)`on`[method](/docs/react-motion-value#on) or [the](/docs/react-use-motion-value-event)`useMotionValueEvent`[hook](/docs/react-use-motion-value-event).

```
useMotionValueEvent(x, "change", (latest) => console.log(latest))
```

Available events are `"change"`, `"animationStart"`, `"animationComplete"` `"animationCancel"`.

### Composition

Beyond `useMotionValue`, Motion provides a number of hooks for creating and composing motion values, like `useSpring` and `useTransform`.

For example, with `useTransform` we can take the latest state of one or more motion values and create a new motion value with the result.

```
const y = useTransform(() => x.get() * 2)
```

`useSpring` can make a motion value that's attached to another via a spring.

```
const dragX = useMotionValue(0)
const dragY = useMotionValue(0)
const x = useSpring(dragX)
const y = useSpring(dragY)
```

*Live example:* https://examples.motion.dev/react/shared-layout-animation?utm_source=embed

These motion values can then go on to be passed to `motion` components, or composed with more hooks like `useVelocity`.

>Motion+ · AI KitMake your AI agent a Motion expert.Give it current Motion context, MotionScore for Agents, and production-ready CSS spring generation.Part of Motion+. One-time fee, lifetime access.›/motion create a photos carousel on homepage- Searching Motion documentation for carousel- Searching Motion examples for “photo carousel”- Found 1 doc and 3 examples.Building new carousel using yourexisting design system.

## API

### get()

Returns the latest state of the motion value.

### getVelocity()

Returns the latest velocity of the motion value. Returns `0` if the value is non-numerical.

### set()

Sets the motion value to a new state.

```
x.set("#f00")
```

### jump()

Jumps the motion value to a new state in a way that breaks continuity from previous values:

- Resets `velocity` to `0`.
- Ends active animations.
- Ignores attached effects (for instance `useSpring`'s spring).

```
const x = useSpring(0)
x.jump(10)
x.getVelocity() // 0
```

### isAnimating()

Returns `true` if the value is currently animating.

### stop()

Stop the active animation.

### on()

Subscribe to motion value events. Available events are:

- `change`
- `animationStart`
- `animationCancel`
- `animationComplete`

It returns a function that, when called, will unsubscribe the listener.

```
const unsubscribe = x.on("change", latest => console.log(latest))
```

When calling `on` inside a React component, it should be wrapped with a `useEffect` hook, or instead use [the](/docs/react-use-motion-value-event)`useMotionValueEvent`[hook](/docs/react-use-motion-value-event).

### destroy()

Destroy and clean up subscribers to this motion value.

This is normally handled automatically, so this method is only necessary if you've manually created a motion value outside the React render cycle using the vanilla `motionValue` hook.
# useScroll

Presented by

Advertise in this space

`useScroll` is used to create scroll-linked animations, like progress indicators and parallax effects.

```
const { scrollYProgress } = useScroll()

return <motion.div style={{ scaleX: scrollYProgress }} />
```

`useScroll` is able to run some animations with the browser's `ScrollTimeline`[API](https://developer.mozilla.org/en-US/docs/Web/API/ScrollTimeline) for optimal hardware-accelerated performance, removing scroll measurements, improving scroll synchronisation and ensuring animations remain smooth even under heavy CPI usage.

## Usage

Import `useScroll` from Motion:

```
import { useScroll } from "motion/react"
```

`useScroll` returns four [motion values](/docs/react-motion-value):

- `scrollX`/`Y`: The absolute scroll position, in pixels.
- `scrollXProgress`/`YProgress`: The scroll position between the defined offsets, as a value between `0` and `1`.

### Page scroll

By default, useScroll tracks the page scroll.

```
const { scrollY } = useScroll()

useMotionValueEvent(scrollY, "change", (latest) => {
  console.log("Page scroll: ", latest)
})
```

For example, we could show a page scroll indicator by passing `scrollYProgress` straight to the `scaleX` style of a progress bar.

```
const { scrollYProgress } = useScroll()

return <motion.div style={{ scaleX: scrollYProgress }} />
```

*Live example:* https://examples.motion.dev/react/scroll-linked?utm_source=embed

As `useScroll` returns motion values, we can compose this scroll info with other motion value hooks like `useTransform` and `useSpring`:

```
const { scrollYProgress } = useScroll()
const scaleX = useSpring(scrollYProgress)

return <motion.div style={{ scaleX }} />
```

*Live example:* https://examples.motion.dev/react/scroll-linked-with-spring?utm_source=embed

> Since scrollY is a MotionValue, there's a neat trick you can use to tell when the user's scroll direction changes: const { scrollY } = useScroll() const [scrollDirection, setScrollDirection] = useState("down") useMotionValueEvent(scrollY, "change", (current) => { const diff = current - scrollY.getPrevious() setScrollDirection(diff > 0 ? "down" : "up") }) Perfect for triggering a sticky header animation! ~ Sam Selikoff, Motion for React Recipes

### Element scroll

To track the scroll position of a scrollable element we can pass the element's `ref` to `useScroll`'s `container` option:

```
const carouselRef = useRef(null)
const { scrollX } = useScroll({
  container: carouselRef
})

return (
  <div ref={carouselRef} style={{ overflow: "scroll" }}>
    {children}
  </div>
)
```

*Live example:* https://examples.motion.dev/react/scroll-container?utm_source=embed

### Element position

We can track the progress of an element as it moves within a container by passing its `ref` to the `target` option.

```
const ref = useRef(null)
const { scrollYProgress } = useScroll({
  target: ref,
  offset: ["start end", "end end"]
})

return <div ref={ref}>
```

In this example, each item has its own progress indicator.

*Live example:* https://examples.motion.dev/react/scroll-track-element-in-viewport?utm_source=embed

### Scroll offsets

With [the](/docs/react-use-scroll#offset)`offset`[option](/docs/react-use-scroll#offset) we can define which parts of the element we want to track with the viewport, for instance track elements as they enter in from the bottom, leave at the top, or travel throughout the whole viewport.

## Performance

Browsers are capable of animating some values, like `opacity`, `transform`, `clipPath` and `filter`, entirely on the GPU. This improves scroll synchronisation and ensures animations remain smooth even when sites are performing heavy work.

`useScroll` is also capable of running animations via the GPU. By passing `scrollXProgress` or `scrollYProgress` either directly to an `opacity` style, or via `useTransform` to one of the above styles, it will create a hardware-accelerated animation.

```
const { scrollYProgress } = useScroll()
const filter = useTransform(scrollYProgress, [0, 1], ["blur(10px)", "blur(0px)"])

return <motion.div style={{ opacity: scrollYProgress, filter }} />
```

## Options

`useScroll` accepts the following options.

### container

**Default**: Viewport

The scrollable container to track the scroll position of. By default, this is the browser viewport. By passing a ref to a scrollable element, that element can be used instead.

```
const containerRef = useRef(null)
const { scrollYProgress } = useScroll({ container: containerRef })
```

### target

`useScroll` tracks the progress of the `target` within the `container`. By default, the `target` is the scrollable area of the `container`. It can additionally be set as another element, to track its progress within the `container`.

```
const targetRef = useRef(null)
const { scrollYProgress } = useScroll({ target: targetRef })
```

`target` is tracked by the element's layout position, so any CSS `transform` applied to it (or its ancestors) is ignored when measuring progress.

### axis

**Default:**`"y"`

The tracked axis for the defined `offset`.

### offset

**Default:** `["start start", "end end"]`

`offset` describes intersections, points where the `target` and `container` meet.

For example, the intersection `"start end"` means when the **start of the target** on the tracked axis meets the **end of the container.**

So if the target is an element, the container is the window, and we're tracking the vertical axis then `"start end"` is where the **top of the element** meets **the bottom of the viewport**.

#### Accepted intersections

Both target and container points can be defined as:

- **Number:** A value where `0` represents the start of the axis and `1` represents the end. So to define the top of the target with the middle of the container you could define `"0 0.5"`. Values outside this range are permitted.
- **Names:** `"start"`, `"center"` and `"end"` can be used as clear shortcuts for `0`, `0.5` and `1` respectively.
- **Pixels:** Pixel values like `"100px"`, `"-50px"` will be defined as that number of pixels from the start of the target/container.
- **Percent:** Same as raw numbers but expressed as `"0%"` to `"100%"`.
- **Viewport:** `"vh"` and `"vw"` units are accepted.

```
// Track an element as it enters from the bottom
const { scrollYProgress } = useScroll({
  target: targetRef,
  offset: ["start end", "end end"]
})

// Track an element as it moves out the top
const { scrollYProgress } = useScroll({
  target: targetRef,
  offset: ["start start", "end start"]
})
```

### trackContentSize

**Default:** `false`

When the size of a page or element's content changes, its scrollable area can change too. But, because browsers don't provide a callback for changes in content size, by default `useScroll()` will not update until the next `"scroll"` event.

`useScroll` can automatically track changes to content size by setting `trackContentSize` to `true`.

```
useScroll({ trackContentSize: true })
```
# useSpring

Presented by+

Advertise in this space

`useSpring` creates [a motion value](/docs/react-motion-value) that will animate to its latest target with a spring animation.

The target can either be set manually via `.set`, or automatically by passing in another motion value.

*Live example:* https://examples.motion.dev/react/follow-pointer-with-spring?utm_source=embed

## Usage

Import `useSpring` from Motion:

```
import { useSpring } from "motion/react"
```

### Direct control

`useSpring` can be created with a number, or a unit-type (`px`, `%` etc) string:

```
const x = useSpring(0)
const y = useSpring("100vh")
```

Now, whenever this motion value is updated via `set()`, the value will animate to its new target with the defined spring.

```
x.set(100)
y.set("50vh")
```

It's also possible to update this value immediately, without a spring, with [the](/docs/react-motion-value#jump)`jump()`[method](/docs/react-motion-value#jump).

```
x.jump(50)
y.jump("0vh")
```

### Track another motion value

Its also possible to automatically spring towards the latest value of another motion value:

```
const x = useMotionValue(0)
const y = useSpring(x)
```

This source motion value must also be a number, or unit-type string.

### Transition

The type of `spring` can be defined with the usual [spring transition option](/docs/react-transitions#spring).

```
useSpring(0, { stiffness: 300 })
```

## Options

As well as transition options, `useSpring` also accepts the following options.

### skipInitialAnimation

**Default:**`false`

When using `useSpring` to track a value like `useScroll`, which may change on mount after a DOM measurement, you can jump to this value instantly by setting `skipInitialAnimation` to `true`.

```
const { scrollYProgress } = useScroll()
const smoothProgress = useSpring(scrollYProgress, {
  skipInitialAnimation: true,
})
```

card.css/motion-appcard.cssCard.tsx1.card {2 transition: scale 200ms linear(3 0, 0.009, 0.036, 0.084, 0.157, 0.255, 0.378,4 0.522, 0.679, 0.832, 0.954, 1.029, 1.052, 1.038,5 1.011, 0.99, 0.984, 0.991, 1.001, 1.005, 16 );7}89.card:hover {10 scale: 1.2;11}MOTIONEaseSpringDuration0.3Delay0›Saved transitions12Visual editing for your agent.Edit and preview Motion and CSS transitions live in your code. Tune ease curves, springs, and durations without leaving your editor.Part of Motion AI Kit. One-time fee, lifetime access.
# useTransform

Presented by+

Advertise in this space

`useTransform` creates a new motion value that transforms the output of one or more motion values.

```
const x = useMotionValue(1)
const y = useMotionValue(1)

const z = useTransform(() => x.get() + y.get()) // z.get() === 2
```

## Usage

Import from Motion:

```
import { useTransform } from "motion/react"
```

`useTransform` can be used in two ways: with a transform function and via value maps:

```
// Transform function
const doubledX = useTransform(() => x.get() * 2)

// Value mapping
const color = useTransform(x, [0, 100], ["#f00", "#00f"])
```

### Transform function

A transform function is a normal function that returns a value.

```
const doubledX = useTransform(() => x.get() * 2)
```

Any motion values read in this function via the `get()` method will be automatically subscribed to.

When these motion values change, the function will be run again on the next animation frame to calculate a new value.

```
const distance = 100
const time = useTime()
const y = useTransform(() => Math.sin(time.get() / 1000) * distance)
```

*Live example:* https://examples.motion.dev/react/use-transform?utm_source=embed

### Value mapping

`useTransform` can also map a single motion value from one range of values to another.

To illustrate, look at this `x` motion value:

```
const x = useMotionValue(0)
```

We can use `useTransform` to create a new motion value called `opacity`.

```
const opacity = useTransform(x, input, output)
```

By defining an `input` range and an `output` range, we can define relationships like "when `x` is `0`, `opacity` should be `1`. When `x` is `100` pixels either side, `opacity` should be `0`".

```
const input = [-100, 0, 100]
const output = [0, 1, 0]
```

Both ranges can be **any length** but must be the **same length** as each other.

The input range must always be a series of increasing or decreasing numbers.

The output range must be values all of the same type, but can be in any order. It can also be any [value type that Motion can animate](/docs/react-animation#animatable-values), like numbers, units, colors and other strings.

```
const backgroundColor = useTransform(
  x,
  [0, 100],
  ["#f00", "#00f"]
)
```

By setting `clamp: false`, the ranges will map perpetually. For instance, in this example we're saying "for every `100px` scrolled, rotate another `360deg`":

```
const { scrollY } = useScroll()
const rotate = useTransform(
  scrollY,
  [0, 100],
  [0, 360],
  { clamp: false }
)
```

#### Output multiple values

It's common to map a single motion value and input range into multiple motion values.

```
const opacity = useTransform(offset, [100, 600], [1, 0.4])
const scale = useTransform(offset, [100, 600], [1, 0.6])
const filter = useTransform(offset, [100, 600], ["blur(0px)", "blur(10px)"])
```

This can lead to some repetition, so `useTransform` also supports mapping to multiple motion values in a single call, by providing a named map:

```
const { opacity, scale, filter } = useTransform(offset, [100, 600], {
  opacity: [1, 0.4],
  scale: [1, 0.6],
  filter: ["blur(0px)", "blur(10px)"],
})
```

## Options

With value mapping, we can set some additional options.

### clamp

**Default:** `true`

If `true`, will clamp output to within the provided range. If `false`, will carry on mapping even when the input falls outside the provided range.

```
const y = useTransform(x, [0, 1], [0, 2])
const z = useTransform(x, [0, 1], [0, 2], { clamp: false })

useEffect(() => {
  x.set(2)
  console.log(y.get()) // 2, input clamped
  console.log(z.get()) // 4
})
```

### ease

An easing function, or array of easing functions, to ease the mixing between each value.

These must be JavaScript functions.

```
import { cubicBezier, circOut } from "motion"
import { useTransform } from "motion/react"

// In your component
const y = useTransform(x, [0, 1], [0, 2], { ease: circOut })

const z = useTransform(
  x,
  [0, 1],
  [0, 2],
  { ease: cubicBezier(0.17, 0.67, 0.83, 0.67) }
)
```

### mixer

A function to use to mix between each pair of output values.

This function will be called with each pair of output values and must return a new function, that accepts a progress value between `0` and `1` and returns the mixed value.

This can be used to inject more advanced mixers than Framer Motion's default, for instance [Flubber](https://github.com/veltman/flubber) for morphing SVG paths.

*Live example:* https://examples.motion.dev/react/path-morphing?utm_source=embed
# useVelocity

Presented by+

Advertise in this space

`useVelocity` accepts a [motion value](/docs/react-motion-value) and returns a new one that updates with the provided motion value's velocity.

```
const x = useMotionValue(0)
const xVelocity = useVelocity(x)
const scale = useTransform(
  xVelocity,
  [-3000, 0, 3000],
  [2, 1, 2],
  { clamp: false }
)

return <motion.div drag="x" style={{ x, scale }} />
```

## Usage

Import `useVelocity` from Motion:

```
import { useVelocity } from "motion/react"
```

Pass any numerical motion value to `useVelocity`. It'll return a new motion value that updates with the velocity of the original value.

```
import { useMotionValue, useVelocity } from "framer-motion"

function Component() {
  const x = useMotionValue(0)
  const xVelocity = useVelocity(x)

  useMotionValueEvent(xVelocity, "change", latest => {
    console.log("Velocity", latestVelocity)
  })
  
  return <motion.div style={{ x }} />
}
```

Any numerical motion value will work. Even one returned from `useVelocity`.

```
const x = useMotionValue(0)
const xVelocity = useVelocity(x)
const xAcceleration = useVelocity(xVelocity)
```
