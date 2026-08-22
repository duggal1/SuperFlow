import { commands, type ModelInfo } from "@/bindings";
import { isLegacySource } from "@/components/onboarding/ModelCard";

/**
 * Spec-aware model recommendation.
 *
 * Everything here is computed from real data: the catalog ships per-model
 * `accuracy_score` and `speed_score` (0..1), and the hardware profile comes
 * from the backend's accelerator probe (real GPU VRAM). No hardcoded
 * "recommended" flags decide the outcome — the catalog flag is only used as a
 * last resort when a catalog entry has no usable scores at all.
 */

export interface HardwareProfile {
  /** Largest GPU VRAM in MB from the accelerator probe, or null when unknown. */
  totalVramMb: number | null;
}

const clamp01 = (n: number) => Math.min(1, Math.max(0, n));

/** Probe the user's machine for the specs that shape real model performance. */
export async function getHardwareProfile(): Promise<HardwareProfile> {
  try {
    const accelerators = await commands.getAvailableAccelerators();
    const vram = accelerators.gpu_devices.reduce(
      (max, device) => Math.max(max, device.total_vram_mb),
      0,
    );
    return { totalVramMb: vram > 0 ? vram : null };
  } catch {
    return { totalVramMb: null };
  }
}

/**
 * Rated speed adjusted for this machine's memory.
 *
 * A model that fits in VRAM runs at its rated speed; every byte that spills
 * to shared memory decodes slower — super-linearly, because hybrid inference
 * shuttles weights across the bus every step. On CPU-only machines the decode
 * cost grows with model footprint; ~500MB is the comfortable realtime
 * reference point.
 */
export function effectiveSpeedScore(
  model: ModelInfo,
  hw: HardwareProfile,
): number {
  const rated = clamp01(model.speed_score);
  if (rated <= 0) return 0;

  if (hw.totalVramMb !== null) {
    // Leave headroom for activations/KV cache — 90% of VRAM is the budget.
    const budgetMb = hw.totalVramMb * 0.9;
    if (model.size_mb <= budgetMb) return rated;
    const residentRatio = budgetMb / model.size_mb; // 0..1, lower = more offload
    return rated * Math.max(residentRatio * residentRatio, 0.05);
  }

  const referenceMb = 500;
  return rated * clamp01(Math.sqrt(referenceMb / Math.max(model.size_mb, 1)));
}

/**
 * Accuracy-to-speed ratio. The harmonic mean punishes imbalance: a model
 * must be BOTH fast and accurate to win. An extremely fast but sloppy model
 * and a pinpoint but sluggish model both lose to a balanced one — which is
 * exactly what a daily-driver transcription model should be.
 */
export function recommendationScore(
  model: ModelInfo,
  hw: HardwareProfile,
): number {
  const accuracy = clamp01(model.accuracy_score);
  const speed = effectiveSpeedScore(model, hw);
  if (accuracy <= 0 || speed <= 0) return 0;
  return (2 * accuracy * speed) / (accuracy + speed);
}

const isEligible = (model: ModelInfo) =>
  !model.is_downloaded && !model.is_custom && !isLegacySource(model);

/**
 * The single best model for THIS machine: highest accuracy-to-speed ratio
 * after accounting for real hardware. Returns null when nothing is
 * downloadable; falls back to the catalog's editorial pick only when the
 * catalog shipped no usable scores.
 */
export function pickRecommendedModel(
  models: ModelInfo[],
  hw: HardwareProfile,
): ModelInfo | null {
  const eligible = models.filter(isEligible);
  if (eligible.length === 0) return null;

  let best: ModelInfo | null = null;
  let bestScore = 0;
  for (const model of eligible) {
    const score = recommendationScore(model, hw);
    if (score > bestScore) {
      best = model;
      bestScore = score;
    }
  }

  if (!best) {
    return eligible.find((m) => m.is_recommended) ?? null;
  }
  return best;
}
