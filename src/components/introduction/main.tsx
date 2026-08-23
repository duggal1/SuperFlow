import { useEffect } from "react";
import {
  motion,
  useMotionValue,
  useReducedMotion,
  useSpring,
  useTransform,
} from "motion/react";
import LightTunnel from "@/components/introduction/LightTunnel";
import { Button } from "@/components/ui/Button";
import { useTranslation } from "react-i18next";

interface IntroductionProps {
  /** Advance the onboarding flow (introduction → permission page). */
  onComplete: () => void;
}

export function Introduction({ onComplete }: IntroductionProps) {
  const { t } = useTranslation();
  const shouldReduceMotion = useReducedMotion();

  // Play the spoken introduction exactly once. Autoplay is attempted first;
  // if the webview blocks it until a user gesture, the first interaction
  // starts it. Leaving the page (Get started) stops it immediately.
  useEffect(() => {
    const audio = new Audio("/introduction.wav");
    let started = false;

    const start = () => {
      if (started) return;
      started = true;
      void audio.play().catch(() => {});
    };

    const armOnFirstGesture = () => {
      window.addEventListener("pointerdown", start, { once: true });
      window.addEventListener("keydown", start, { once: true });
    };

    audio
      .play()
      .then(() => {
        started = true;
      })
      .catch(armOnFirstGesture);

    return () => {
      window.removeEventListener("pointerdown", start);
      window.removeEventListener("keydown", start);
      audio.pause();
      audio.currentTime = 0;
    };
  }, []);

  const pointerX = useMotionValue(0);
  const pointerY = useMotionValue(0);

  // Slow, heavy springs: the parallax glides rather than tracks the pointer.
  const springX = useSpring(pointerX, {
    stiffness: 40,
    damping: 22,
    mass: 1.1,
  });

  const springY = useSpring(pointerY, {
    stiffness: 40,
    damping: 22,
    mass: 1.1,
  });

  const contentX = useTransform(springX, [-1, 1], [-4, 4]);
  const contentY = useTransform(springY, [-1, 1], [-3, 3]);

  const logoX = useTransform(springX, [-1, 1], [-7, 7]);
  const logoY = useTransform(springY, [-1, 1], [-5, 5]);

  useEffect(() => {
    if (shouldReduceMotion) return;

    const handlePointerMove = (event: PointerEvent) => {
      const x = (event.clientX / window.innerWidth - 0.5) * 2;
      const y = (event.clientY / window.innerHeight - 0.5) * 2;

      pointerX.set(x);
      pointerY.set(y);
    };

    const handlePointerLeave = () => {
      pointerX.set(0);
      pointerY.set(0);
    };

    window.addEventListener("pointermove", handlePointerMove);
    document.documentElement.addEventListener(
      "pointerleave",
      handlePointerLeave,
    );

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      document.documentElement.removeEventListener(
        "pointerleave",
        handlePointerLeave,
      );
    };
  }, [pointerX, pointerY, shouldReduceMotion]);

  return (
    <main className="relative min-h-dvh cursor-default select-none overflow-hidden bg-stone-900 font-sans antialiased tracking-tight">
      {/* Tunnel */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-0"
      >
        <LightTunnel
          cableColor="#150dff"
          pulseColor="#0f03fc"
          tunnelColor="#0b00d6"
          tunnelOpacity={0}
          speed={0.1}
          flowDirection="outward"
          pulseSpeed={2}
          pulseLength={0.28}
          pulseBlend={1}
          pulseWidth={1}
          cableCount={20}
          thickness={0.35}
          rimWidth={0.15}
          waviness={0.3}
          sway={0.5}
          size={1}
          centerX={0}
          centerY={0}
          glow={1}
          fadeNear={0.5}
          fadeFar={2}
          brightness={1}
          colorVariance
          grain
          grainIntensity={0.05}
          opacity={1}
          mouseInteraction
          mouseStrength={0.1}
        />
      </div>

      {/* Very subtle center depth */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-1 bg-[radial-gradient(circle_at_center,transparent_0%,rgba(28,25,23,0.08)_38%,rgba(28,25,23,0.72)_100%)]"
      />

      {/* Content */}
      <div className="relative z-10 flex min-h-dvh items-center justify-center px-6">
        <motion.div
          style={
            shouldReduceMotion
              ? undefined
              : {
                  x: contentX,
                  y: contentY,
                }
          }
          initial={
            shouldReduceMotion
              ? false
              : {
                  opacity: 0,
                  y: 18,
                  filter: "blur(8px)",
                }
          }
          animate={{
            opacity: 1,
            y: 0,
            filter: "blur(0px)",
          }}
          transition={{
            duration: 1.3,
            ease: [0.16, 1, 0.3, 1],
          }}
          className="flex w-full max-w-155 flex-col items-center text-center"
        >
          {/* Logo */}
          <motion.div
            style={
              shouldReduceMotion
                ? undefined
                : {
                    x: logoX,
                    y: logoY,
                  }
            }
            initial={
              shouldReduceMotion
                ? false
                : {
                    opacity: 0,
                    scale: 0.92,
                    filter: "blur(6px)",
                  }
            }
            animate={{
              opacity: 1,
              scale: 1,
              filter: "blur(0px)",
            }}
            transition={{
              duration: 1.2,
              delay: 0.1,
              ease: [0.16, 1, 0.3, 1],
            }}
            className="mb-7"
          >
            <img
              src="/logo.svg"
              alt="SuperFlow"
              className="size-14 select-none object-contain"
              draggable={false}
            />
          </motion.div>

          {/* Heading */}
          <motion.h1
            initial={
              shouldReduceMotion
                ? false
                : {
                    opacity: 0,
                    y: 12,
                    filter: "blur(5px)",
                  }
            }
            animate={{
              opacity: 1,
              y: 0,
              filter: "blur(0px)",
            }}
            transition={{
              duration: 1.25,
              delay: 0.28,
              ease: [0.16, 1, 0.3, 1],
            }}
            className="text-balance text-[38px] leading-[1.03] font-normal tracking-[-0.045em] text-stone-100 sm:text-[52px]"
          >
            {t("introduction.title")}
          </motion.h1>

          {/* Description */}
          <motion.p
            initial={
              shouldReduceMotion
                ? false
                : {
                    opacity: 0,
                    y: 10,
                    filter: "blur(4px)",
                  }
            }
            animate={{
              opacity: 1,
              y: 0,
              filter: "blur(0px)",
            }}
            transition={{
              duration: 1.25,
              delay: 0.45,
              ease: [0.16, 1, 0.3, 1],
            }}
            className="mt-4.5 max-w-130 text-balance  tracking-tight antialiased text-[16px] leading-7 font-normal  text-stone-200/85 sm:text-[18px]"
          >
            {t("introduction.description")}
          </motion.p>

          {/* CTA */}
          <motion.div
            initial={
              shouldReduceMotion
                ? false
                : {
                    opacity: 0,
                    y: 8,
                    scale: 0.97,
                  }
            }
            animate={{
              opacity: 1,
              y: 0,
              scale: 1,
            }}
            transition={{
              duration: 1.1,
              delay: 0.62,
              ease: [0.16, 1, 0.3, 1],
            }}
            className="mt-7"
          >
            <Button
              onClick={onComplete}
              variant="primary"
              className="h-7.5 mt-1 rounded-[7px] font-normal border-[#150dff] bg-[#150dff] px-6.5  hover:border-[#0f03fc] hover:bg-[#0f03fc]/90"
            >
              {t("introduction.getStarted")}
            </Button>
          </motion.div>
        </motion.div>
      </div>
    </main>
  );
}
