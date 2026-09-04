import { Channel, invoke } from "@tauri-apps/api/core";

type TtsStreamEvent =
  | { event: "started"; sample_rate: number }
  | { event: "chunk"; samples: number[] }
  | { event: "finished"; duration_ms: number; first_audio_ms: number };

interface TtsStatus {
  engine_available: boolean;
  model_downloaded: boolean;
}

const MAX_SPEAK_CHARS = 300;
const SAMPLE_RATE = 24_000;
const LEAD_SECONDS = 0.025;

let generation = 0;
let audioContext: AudioContext | null = null;
let scheduledSources = new Set<AudioBufferSourceNode>();
let nextStartTime = 0;

function ensureContext(): AudioContext {
  if (!audioContext || audioContext.state === "closed") {
    audioContext = new AudioContext({ sampleRate: SAMPLE_RATE });
    scheduledSources = new Set();
    nextStartTime = 0;
  }
  return audioContext;
}

if (typeof window !== "undefined") {
  const unlock = () => {
    if (audioContext && audioContext.state === "suspended") {
      void audioContext.resume().catch(() => {});
    }
  };
  window.addEventListener("pointerdown", unlock);
  window.addEventListener("keydown", unlock);
}

function stopScheduledSources() {
  generation += 1;
  for (const source of scheduledSources) {
    try {
      source.stop();
    } catch {
      // Already ended.
    }
  }
  scheduledSources.clear();
  nextStartTime = 0;
}

export function stopAgentSpeech(): void {
  stopScheduledSources();
  if (audioContext && audioContext.state === "suspended") {
    void audioContext.resume().catch(() => {});
  }
}

/**
 * Speak one agent status string through the selected Pocket-TTS voice using
 * the same streaming machinery as the voice preview (warm server, chunked
 * synthesis, gapless WebAudio scheduling). New speech cancels old speech.
 * Silent no-op when the TTS model is not downloaded. Never throws.
 */
export async function speakAgentStatus(
  rawText: string | null | undefined,
): Promise<void> {
  const text = (rawText ?? "").trim().slice(0, MAX_SPEAK_CHARS).trim();
  if (!text) return;
  stopScheduledSources();
  const myGeneration = generation;
  try {
    const status = await invoke<TtsStatus>("tts_status");
    if (myGeneration !== generation) return;
    if (!status.engine_available || !status.model_downloaded) return;
    const context = ensureContext();
    await context.resume().catch(() => {});
    if (myGeneration !== generation) return;
    nextStartTime = context.currentTime;
    const channel = new Channel<TtsStreamEvent>();
    channel.onmessage = (event) => {
      if (myGeneration !== generation) return;
      if (event.event !== "chunk") return;
      const samples = Float32Array.from(event.samples, (s) => s / 32768);
      if (samples.length === 0) return;
      const buffer = context.createBuffer(1, samples.length, SAMPLE_RATE);
      buffer.copyToChannel(samples, 0);
      const source = context.createBufferSource();
      source.buffer = buffer;
      source.connect(context.destination);
      const startAt = Math.max(context.currentTime + LEAD_SECONDS, nextStartTime);
      nextStartTime = startAt + buffer.duration;
      scheduledSources.add(source);
      source.onended = () => {
        scheduledSources.delete(source);
      };
      source.start(startAt);
    };
    await invoke("tts_synthesize", { text, onEvent: channel });
  } catch (e) {
    console.error("speakAgentStatus failed", e);
  }
}
