/* eslint-disable i18next/no-literal-string */
"use client";

import { useState } from "react";
import { LiveWaveform } from "./live-waveform";
import { Button } from "../ui/Button";

export function LiveWaveformDemo() {
  const [active, setActive] = useState(false);
  const [processing, setProcessing] = useState(false);
  const [mode, setMode] = useState<"static" | "scrolling">("static");

  const handleToggleActive = () => {
    setActive(!active);
    if (!active) setProcessing(false);
  };

  const handleToggleProcessing = () => {
    setProcessing(!processing);
    if (!processing) setActive(false);
  };

  return (
    <div className="w-full rounded-[6px] border border-white/[0.06] bg-aqua-900 p-6">
      <div className="mb-4">
        <h3 className="text-[14px] font-[550] tracking-[-0.02em] text-aqua-50">
          Live Audio Waveform
        </h3>
        <p className="mt-1 text-xs leading-5 text-aqua-50/55">
          Real-time microphone input — aqua ultra-clean, centered waveform
        </p>
      </div>

      <div className="space-y-4">
        <div className="overflow-hidden rounded-[6px] border border-white/[0.06] bg-aqua-950 p-2">
          <LiveWaveform
            active={active}
            processing={processing}
            height={72}
            barWidth={2.5}
            barGap={1.5}
            barRadius={1.5}
            mode={mode}
            fadeEdges
            barColor="#0400e3"
            historySize={120}
          />
        </div>

        <div className="flex flex-wrap justify-center gap-2">
          <Button
            variant={active ? "primary" : "secondary"}
            onClick={handleToggleActive}
          >
            {active ? "Stop" : "Start"} Listening
          </Button>
          <Button
            variant={processing ? "primary" : "secondary"}
            onClick={handleToggleProcessing}
          >
            {processing ? "Stop" : "Start"} Processing
          </Button>
          <Button
            variant="secondary"
            onClick={() => setMode(mode === "static" ? "scrolling" : "static")}
          >
            Mode: {mode === "static" ? "Static" : "Scrolling"}
          </Button>
        </div>
      </div>
    </div>
  );
}
