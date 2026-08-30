#!/usr/bin/env python3

from __future__ import annotations

import argparse
import base64
import gc
import json
import os
import struct
import subprocess
import sys
import tempfile
import time
import wave
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import mlx.core as mx
import numpy as np

from mlx_audio.stt import load as load_asr
from mlx_lm import generate as llm_generate
from mlx_lm import load as load_llm


# ============================================================================
# Models
# ============================================================================

Engine = Literal["mlx-audio", "parakeet-mlx", "ark"]
StreamType = Literal[
    "none",
    "token",
    "streaming-asr",
]


@dataclass(frozen=True)
class ASRModel:
    alias: str
    model_id: str
    engine: Engine

    approximate_weight_gb: float

    streaming: StreamType

    multilingual: bool

    accuracy_score: int
    speed_score: int

    notes: str


MODELS: dict[str, ASRModel] = {
    # -----------------------------------------------------------------------
    # Qwen3-ASR
    # -----------------------------------------------------------------------

    "qwen-0.6b": ASRModel(
        alias="qwen-0.6b",
        model_id="mlx-community/Qwen3-ASR-0.6B-8bit",
        engine="mlx-audio",
        approximate_weight_gb=0.8,
        streaming="token",
        multilingual=True,
        accuracy_score=8,
        speed_score=8,
        notes=(
            "Best balanced Qwen ASR. "
            "Small, multilingual, LLM decoder."
        ),
    ),

    "qwen-1.7b": ASRModel(
        alias="qwen-1.7b",
        model_id="mlx-community/Qwen3-ASR-1.7B-8bit",
        engine="mlx-audio",
        approximate_weight_gb=2.0,
        streaming="token",
        multilingual=True,
        accuracy_score=9,
        speed_score=6,
        notes=(
            "Accuracy-first Qwen3-ASR. "
            "Use when RAM headroom is available."
        ),
    ),
    "qwen-1.7b-4bit": ASRModel(
        alias="qwen-1.7b-4bit",
        model_id="mlx-community/Qwen3-ASR-1.7B-4bit",
        engine="mlx-audio",
        approximate_weight_gb=1.6,
        streaming="token",
        multilingual=True,
        accuracy_score=8,
        speed_score=7,
        notes=(
            "Qwen3-ASR accuracy with lower RAM footprint. "
            "4-bit affine quantization, group size 64."
        ),
    ),

    "qwen-1.7b-bf16": ASRModel(
        alias="qwen-1.7b-bf16",
        model_id="mlx-community/Qwen3-ASR-1.7B-bf16",
        engine="mlx-audio",
        approximate_weight_gb=4.2,
        streaming="token",
        multilingual=True,
        accuracy_score=10,
        speed_score=5,
        notes=(
            "Full-precision BF16 fidelity. "
            "Largest RAM requirement; use only on big-memory Macs."
        ),
    ),

    # Aliases matching the SuperFlow Rust MlxVariant cli_alias values.
    "qwen-1.7b-8bit": ASRModel(
        alias="qwen-1.7b-8bit",
        model_id="mlx-community/Qwen3-ASR-1.7B-8bit",
        engine="mlx-audio",
        approximate_weight_gb=2.0,
        streaming="token",
        multilingual=True,
        accuracy_score=9,
        speed_score=6,
        notes=(
            "High-accuracy multilingual Qwen3-ASR (8-bit). "
            "Matches the qwen-1.7b checkpoint weight-for-weight."
        ),
    ),
    "qwen-0.6b-8bit": ASRModel(
        alias="qwen-0.6b-8bit",
        model_id="mlx-community/Qwen3-ASR-0.6B-8bit",
        engine="mlx-audio",
        approximate_weight_gb=0.8,
        streaming="token",
        multilingual=True,
        accuracy_score=9,
        speed_score=9,
        notes=(
            "Small, fast multilingual Qwen3-ASR (8-bit). "
            "Best balance of speed and accuracy."
        ),
    ),

    # -----------------------------------------------------------------------
    # Parakeet
    # -----------------------------------------------------------------------

    "parakeet-test": ASRModel(
        alias="parakeet-test",
        model_id="animaslabs/parakeet-tdt-0.6b-v3-mlx-4bit",
        engine="mlx-audio",
        approximate_weight_gb=0.6,
        streaming="streaming-asr",
        multilingual=True,
        accuracy_score=8,
        speed_score=10,
        notes=(
            "Small 4-bit Parakeet used for validating "
            "the MLX/Metal pipeline end-to-end."
        ),
    ),

    # -----------------------------------------------------------------------
    # Parakeet
    # -----------------------------------------------------------------------

    "parakeet": ASRModel(
        alias="parakeet",
        model_id="mlx-community/parakeet-tdt-0.6b-v3",
        engine="mlx-audio",
        approximate_weight_gb=1.3,
        streaming="streaming-asr",
        multilingual=True,
        accuracy_score=8,
        speed_score=10,
        notes=(
            "Excellent fast dictation model. "
            "Very strong choice for live speech."
        ),
    ),

    "parakeet-unified": ASRModel(
        alias="parakeet-unified",
        model_id="animaslabs/parakeet-tdt-0.6b-v3-mlx-8bit",
        engine="parakeet-mlx",
        approximate_weight_gb=0.908,
        streaming="none",
        multilingual=False,
        accuracy_score=0,
        speed_score=10,
        notes=(
            "English Parakeet TDT 0.6B v3 with an INT8 encoder."
        ),
    ),

    # -----------------------------------------------------------------------
    # Nemotron streaming
    # -----------------------------------------------------------------------

    "nemotron": ASRModel(
        alias="nemotron",
        model_id=(
            "mlx-community/"
            "nemotron-3.5-asr-streaming-0.6b-8bit"
        ),
        engine="mlx-audio",
        approximate_weight_gb=0.8,
        streaming="streaming-asr",
        multilingual=True,
        accuracy_score=8,
        speed_score=9,
        notes=(
            "Cache-aware FastConformer/RNNT architecture. "
            "Designed for streaming ASR."
        ),
    ),

    # -----------------------------------------------------------------------
    # MOSS
    # -----------------------------------------------------------------------

    "moss": ASRModel(
        alias="moss",
        model_id="OpenMOSS-Team/MOSS-Transcribe-Diarize",
        engine="mlx-audio",
        approximate_weight_gb=2.0,
        streaming="token",
        multilingual=True,
        accuracy_score=9,
        speed_score=5,
        notes=(
            "Transcription + timestamps + speaker diarization."
        ),
    ),

    # -----------------------------------------------------------------------
    # Whisper
    # -----------------------------------------------------------------------

    "whisper": ASRModel(
        alias="whisper",
        model_id=(
            "mlx-community/"
            "whisper-large-v3-turbo-asr-fp16"
        ),
        engine="mlx-audio",
        approximate_weight_gb=1.6,
        streaming="token",
        multilingual=True,
        accuracy_score=8,
        speed_score=7,
        notes="Extremely mature multilingual fallback.",
    ),

    "cohere": ASRModel(
        alias="cohere",
        model_id="littoralai/cohere-transcribe-mlx-8bit",
        engine="mlx-audio",
        approximate_weight_gb=2.0,
        streaming="none",
        multilingual=True,
        accuracy_score=9,
        speed_score=7,
        notes=(
            "Cohere Transcribe 03-2026 (8-bit). "
            "High-quality multilingual STT; no streaming."
        ),
    ),

    # -----------------------------------------------------------------------
    # ARK
    # -----------------------------------------------------------------------

    "ark-0.6b": ASRModel(
        alias="ark-0.6b",
        model_id="leope/ark-asr-0.6B-mlx",
        engine="ark",
        approximate_weight_gb=2.1,
        streaming="none",
        multilingual=True,
        accuracy_score=9,
        speed_score=6,
        notes=(
            "Very accurate native MLX ARK port. "
            "Offline only, <=30 second clips."
        ),
    ),

    "ark-3b": ASRModel(
        alias="ark-3b",
        model_id="leope/ark-asr-3B-mlx",
        engine="ark",
        approximate_weight_gb=8.0,
        streaming="none",
        multilingual=True,
        accuracy_score=10,
        speed_score=3,
        notes=(
            "Accuracy monster. Huge memory requirement. "
            "Offline only."
        ),
    ),
}


# ============================================================================
# LLM
# ============================================================================

DEFAULT_LLM_SMALL = "mlx-community/Qwen3-1.7B-4bit"

DEFAULT_LLM_LARGE = (
    "mlx-community/"
    "Qwen3-4B-Instruct-2507-4bit"
)


CLEANUP_SYSTEM_PROMPT = """
You are a transcript-to-prompt editor.

Convert messy dictated speech into a concise, polished, high-quality
Markdown prompt.

Hard requirements:

- Preserve the user's exact intent.
- Preserve every meaningful requirement and constraint.
- Preserve uncertainty as uncertainty.
- Preserve filenames, paths, function names, code identifiers,
  technologies, numbers, versions, and technical terminology.
- Fix grammar, spelling, punctuation, capitalization, and broken
  dictated sentence structure.
- Remove filler, false starts, verbal clutter, and accidental repetition.
- Deduplicate repeated requirements and ideas.
- Correct obvious speech-recognition mistakes only when the intended
  wording is clear from context.
- Never invent requirements.
- Never add solutions that the user did not request.
- Never answer the user's request.
- Never execute the task.
- Do not explain your editing process.
- Use Markdown only where it improves readability.
- Do not force headings onto trivial one-sentence requests.
- Return only the rewritten prompt.
""".strip()


# ============================================================================
# Hardware
# ============================================================================


def system_ram_gb() -> float:
    try:
        output = subprocess.check_output(
            ["sysctl", "-n", "hw.memsize"],
            text=True,
        ).strip()

        return int(output) / (1024**3)

    except Exception:
        return 0.0


def reset_mlx_memory() -> None:
    gc.collect()

    try:
        mx.clear_cache()
    except Exception:
        pass

    try:
        mx.reset_peak_memory()
    except Exception:
        pass


# ============================================================================
# Model selection
# ============================================================================


def choose_asr(
    mode: str,
    ram_gb: float,
) -> ASRModel:

    if mode == "live":
        # Lowest latency / dictation-oriented.
        return MODELS["parakeet"]

    if mode == "live-multilingual":
        return MODELS["nemotron"]

    if mode == "balanced":
        return MODELS["qwen-0.6b"]

    if mode == "quality":
        if ram_gb >= 12:
            return MODELS["qwen-1.7b"]

        return MODELS["qwen-0.6b"]

    if mode == "diarize":
        return MODELS["moss"]

    if mode == "max-accuracy":
        if ram_gb >= 16:
            return MODELS["ark-3b"]

        return MODELS["ark-0.6b"]

    raise ValueError(
        f"Unknown mode: {mode}"
    )


def choose_llm(ram_gb: float) -> str:
    # Don't murder unified memory for a cleanup operation.
    if ram_gb >= 16:
        return DEFAULT_LLM_LARGE

    return DEFAULT_LLM_SMALL


# ============================================================================
# MLX Audio engine
# ============================================================================


class MLXAudioASR:
    def __init__(self, spec: ASRModel) -> None:
        self.spec = spec
        self.model: Any | None = None

    def load(self) -> None:
        print(
            f"Loading ASR: {self.spec.model_id}",
            file=sys.stderr,
        )

        self.model = load_asr(
            self.spec.model_id
        )

    def transcribe(
        self,
        audio: str,
        language: str | None,
    ) -> str:

        if self.model is None:
            self.load()

        kwargs: dict[str, Any] = {}

        # Qwen language names:
        # English, Chinese, German...
        if self.spec.alias.startswith("qwen"):
            if language:
                kwargs["language"] = language

        # Nemotron uses keys such as en-US.
        elif self.spec.alias == "nemotron":
            kwargs["language"] = language or "auto"

        # MOSS is deterministic by default at temperature 0.
        elif self.spec.alias == "moss":
            kwargs["temperature"] = 0.0
            kwargs["max_tokens"] = 4096

        result = self.model.generate(
            audio,
            **kwargs,
        )

        text = getattr(
            result,
            "text",
            None,
        )

        if not text:
            raise RuntimeError(
                f"No text returned from {self.spec.alias}"
            )

        return str(text).strip()

    def stream_file(
        self,
        audio: str,
        language: str | None,
    ) -> str:

        if self.model is None:
            self.load()

        pieces: list[str] = []

        # Qwen exposes stream_transcribe directly.
        if self.spec.alias.startswith("qwen"):

            kwargs: dict[str, Any] = {}

            if language:
                kwargs["language"] = language

            for text in self.model.stream_transcribe(
                audio,
                **kwargs,
            ):
                value = getattr(
                    text,
                    "text",
                    text,
                )

                value = str(value)

                pieces.append(value)

                print(
                    value,
                    end="",
                    flush=True,
                )

        # Parakeet / MOSS and several mlx-audio models expose
        # stream=True through generate().
        else:

            kwargs = {
                "stream": True,
            }

            if self.spec.alias == "nemotron":
                kwargs["language"] = (
                    language or "auto"
                )

            if self.spec.alias == "moss":
                kwargs["temperature"] = 0.0
                kwargs["max_tokens"] = 4096

            generator = self.model.generate(
                audio,
                **kwargs,
            )

            for chunk in generator:
                value = getattr(
                    chunk,
                    "text",
                    chunk,
                )

                value = str(value)

                pieces.append(value)

                print(
                    value,
                    end="",
                    flush=True,
                )

        print()

        return "".join(pieces).strip()


# ============================================================================
# ARK adapter
# ============================================================================


def transcribe_ark(
    spec: ASRModel,
    audio: str,
) -> str:

    root = Path(__file__).resolve().parent

    ark_python = (
        root
        / ".venv-ark"
        / "bin"
        / "python"
    )

    runner = root / "ark_runner.py"

    if not ark_python.exists():
        raise RuntimeError(
            "ARK environment missing.\n"
            "Run:\n\n"
            "    ./setup_ark.sh\n"
        )

    command = [
        str(ark_python),
        str(runner),
        "--model",
        spec.alias,
        "--audio",
        audio,
    ]

    result = subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
    )

    return result.stdout.strip()


def transcribe_parakeet(
    spec: ASRModel,
    audio: str,
) -> str:
    import json

    import mlx.nn as nn
    from huggingface_hub import hf_hub_download
    from parakeet_mlx.utils import from_config

    config_path = hf_hub_download(spec.model_id, "config.json")
    weights_path = hf_hub_download(spec.model_id, "model.safetensors")
    with open(config_path, encoding="utf-8") as config_file:
        config = json.load(config_file)

    model = from_config(config)
    nn.quantize(
        model,
        bits=config["quantization"]["bits"],
        group_size=config["quantization"]["group_size"],
    )
    model.load_weights(weights_path)
    result = model.transcribe(audio)
    text = getattr(result, "text", None)

    if not text:
        raise RuntimeError(
            f"No text returned from {spec.alias}"
        )

    return str(text).strip()


# ============================================================================
# Cleanup LLM
# ============================================================================


class CleanupLLM:
    def __init__(
        self,
        model_id: str,
    ) -> None:
        self.model_id = model_id
        self.model = None
        self.tokenizer = None

    def load(self) -> None:
        print(
            f"Loading cleanup LLM: {self.model_id}",
            file=sys.stderr,
        )

        self.model, self.tokenizer = load_llm(
            self.model_id
        )

    def generate(
        self,
        user: str,
        max_tokens: int = 1024,
        system: str | None = None,
    ) -> str:

        if self.model is None:
            self.load()

        assert self.tokenizer is not None
        assert self.model is not None

        messages = [
            {
                "role": "system",
                "content": system
                if system is not None
                else CLEANUP_SYSTEM_PROMPT,
            },
            {
                "role": "user",
                "content": user,
            },
        ]

        template_kwargs = dict(
            tokenize=False,
            add_generation_prompt=True,
        )

        try:
            prompt = (
                self.tokenizer
                .apply_chat_template(
                    messages,
                    enable_thinking=False,
                    **template_kwargs,
                )
            )

        except TypeError:
            prompt = (
                self.tokenizer
                .apply_chat_template(
                    messages,
                    **template_kwargs,
                )
            )

        response = llm_generate(
            self.model,
            self.tokenizer,
            prompt=prompt,
            max_tokens=max_tokens,
            verbose=False,
        )

        return response.strip()

    def cleanup(
        self,
        transcript: str,
        max_tokens: int = 1024,
    ) -> str:

        return self.generate(
            transcript,
            max_tokens,
        )


def run_llm_serve(model_id: str) -> None:
    """Long-lived local LLM inference for Rust (JSONL stdin/stdout).

    The model is loaded once and reused for every request, so a sequence of
    AI prompts never pays a reload. One JSON line in:

        {"system": "...", "user": "...", "max_tokens": 1024}

    one JSON line out:

        {"text": "..."}   or   {"error": "..."}
    """

    cleaner = CleanupLLM(model_id)

    # Confirm liveness immediately so Rust can fail fast on a bad model id.
    print(
        json.dumps({"ready": True, "model": model_id}),
        flush=True,
    )

    for line in sys.stdin:

        line = line.strip()

        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as error:
            print(
                json.dumps({"error": f"bad request json: {error}"}),
                flush=True,
            )
            continue

        if request.get("type") == "ping":
            print(
                json.dumps({"text": "pong"}),
                flush=True,
            )
            continue

        system = request.get("system")
        user = request.get("user", "")
        max_tokens = int(request.get("max_tokens", 1024))

        try:
            output = cleaner.generate(
                user,
                max_tokens,
                system=system,
            )
            print(
                json.dumps({"text": output}),
                flush=True,
            )

        except Exception as error:
            print(
                json.dumps({"error": str(error)}),
                flush=True,
            )


# ============================================================================
# Pipeline
# ============================================================================


def transcribe(
    spec: ASRModel,
    audio: str,
    language: str | None,
    stream: bool,
) -> str:

    if spec.engine == "ark":
        if stream:
            raise RuntimeError(
                "ARK MLX port is currently offline only."
            )

        return transcribe_ark(
            spec,
            audio,
        )

    if spec.engine == "parakeet-mlx":
        if stream:
            raise RuntimeError(
                "Parakeet MLX streaming is not supported."
            )

        return transcribe_parakeet(
            spec,
            audio,
        )

    runtime = MLXAudioASR(spec)

    if stream:
        return runtime.stream_file(
            audio,
            language,
        )

    return runtime.transcribe(
        audio,
        language,
    )


# ============================================================================
# Live incremental streaming (Rust → Python JSONL bridge)
# ============================================================================


def _write_pcm_to_wav(samples: list[float], sample_rate: int = 16000) -> str:
    """Write float32 PCM ([-1,1], 16kHz mono) to a temp int16 WAV file. Caller must unlink."""
    # Clamp and convert to int16
    # Use tempfile to avoid collisions; delete=False so mlx_audio can open by path
    fd, path = tempfile.mkstemp(suffix=".wav")
    os.close(fd)
    try:
        with wave.open(path, "wb") as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)  # 16-bit
            wf.setframerate(sample_rate)
            # Pack as little-endian int16
            # Clamp in Python loop is slower but acceptable for <30s buffers; bulk via struct
            ints = [max(-32768, min(32767, int(max(-1.0, min(1.0, s)) * 32767))) for s in samples]
            wf.writeframes(struct.pack(f"<{len(ints)}h", *ints) if ints else b"")
    except Exception:
        try:
            os.unlink(path)
        except Exception:
            pass
        raise
    return path


def run_live(spec: ASRModel, language: str | None) -> None:
    """Long-lived JSONL bridge for live overlay streaming.

    Protocol (mirrors transcribe-cpp Stream feed/finalize):
      stdin  JSONL: {"type":"feed","samples":[...]} | {"type":"finalize"} | {"type":"cancel"}
                samples may be float array, or base64 string of float32 LE bytes
      stdout JSONL: {"committed":"...","tentative":""} | {"committed":"final","tentative":"","is_final":true}
                Errors: {"error":"msg"}
      stderr: diagnostics

    Two paths:
      - Nemotron (streaming-asr): true cache-aware incremental via
        StreamingLogMelSpectrogram + ConformerStreamingState + incremental
        RNNT decode (O(n), no recompute). Mirrors mlx_audio's
        test_voicechat_style_frontend_and_conformer_state_tracks_bounded_encoder.
      - Others (Qwen token-streaming etc): snapshot re-decode of bounded
        30s window every ~400ms, re-using the resident MLXAudioASR.
        Still streaming from UX view, just not cache-aware.
    """
    # ------------------------------------------------------------------
    # Nemotron true incremental path
    # ------------------------------------------------------------------
    if spec.alias == "nemotron":
        # Lazily import heavy nemotron-specific modules only when needed
        try:
            from mlx_audio.stt.models.nemotron_asr.audio import (
                StreamingLogMelSpectrogram,
            )
            from mlx_audio.stt.models.nemotron_asr.streaming import (
                ConformerStreamingState,
            )
            from mlx_audio.stt.models.nemotron_asr import tokenizer as nemo_tok
            from mlx_audio.stt.models.nemo.alignment import AlignedToken
        except Exception as e:
            print(json.dumps({"error": f"nemotron streaming imports failed: {e}"}), flush=True)
            sys.exit(1)

        runtime = MLXAudioASR(spec)
        try:
            runtime.load()
        except Exception as e:
            print(json.dumps({"error": f"model load failed: {e}"}), flush=True)
            sys.exit(1)

        model = runtime.model
        # Attributes from Model (mlx_audio)
        try:
            preprocessor = model.preprocessor_config
            blank_id = model.blank_id
            max_symbols = model.max_symbols
            vocabulary = model.vocabulary
            frame_sec = (
                model.encoder_config.subsampling_factor
                * preprocessor.hop_length
                / preprocessor.sample_rate
            )
        except Exception as e:
            print(json.dumps({"error": f"model attribute probe failed: {e}"}), flush=True)
            sys.exit(1)

        print(json.dumps({"ready": True, "model": spec.alias}), flush=True)
        sys.stderr.write(f"MLX live ready (cache-aware): {spec.alias} language={language}\n")
        sys.stderr.flush()

        frontend = StreamingLogMelSpectrogram(preprocessor)
        # Use model's default att_context_size, e.g. [56,13]
        try:
            att_context = model.default_att_context_size
        except Exception:
            att_context = [56, 13]
        conformer_state = ConformerStreamingState(
            model.encoder, att_context_size=att_context
        )
        # Incremental RNNT decoder state (mirrors Model._decode_prompted_chunks)
        last_token = blank_id
        decoder_hidden = None  # type: ignore[assignment]
        hypothesis: list[Any] = []  # list[AlignedToken]
        global_time = 0

        def _emit_text() -> str:
            try:
                from mlx_audio.stt.models.nemo.alignment import (
                    sentences_to_result,
                    tokens_to_sentences,
                )

                result = sentences_to_result(tokens_to_sentences(hypothesis))
                text = result.text or ""
                # Strip leading language tag if present (<en-US> etc)
                if text.startswith("<") and ">" in text[:12]:
                    # tokenizer decode already strips, but be safe
                    pass
                return text.strip()
            except Exception:
                # Fallback: decode via tokenizer directly
                try:
                    ids = [t.token if hasattr(t, "token") else t for t in hypothesis]  # type: ignore
                    return nemo_tok.decode(ids, vocabulary).strip()  # type: ignore
                except Exception:
                    return ""

        for raw_line in sys.stdin:
            line = raw_line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except Exception as e:
                print(json.dumps({"error": f"bad json: {e}"}), flush=True)
                continue

            mtype = msg.get("type")
            if mtype == "feed":
                samples = msg.get("samples")
                if samples is None:
                    samples = msg.get("pcm", [])
                pcm_chunk: list[float] = []
                if isinstance(samples, str):
                    try:
                        raw_bytes = base64.b64decode(samples)
                        count = len(raw_bytes) // 4
                        if count:
                            pcm_chunk = list(struct.unpack(f"<{count}f", raw_bytes))
                    except Exception as e:
                        print(json.dumps({"error": f"base64 decode failed: {e}"}), flush=True)
                        continue
                elif isinstance(samples, list):
                    pcm_chunk = [float(s) for s in samples if isinstance(s, (int, float))]
                else:
                    print(json.dumps({"error": "feed samples must be list or base64 string"}), flush=True)
                    continue

                if not pcm_chunk:
                    continue

                # Frontend: PCM -> mel frames (incremental, bounded)
                try:
                    pcm_arr = mx.array(np.array(pcm_chunk, dtype=np.float32))
                    mel = frontend.push(pcm_arr)
                except Exception as e:
                    print(json.dumps({"error": f"frontend push failed: {e}"}), flush=True)
                    continue

                if mel is None or getattr(mel, "shape", (0, 0))[1] == 0:
                    # No new mel frames yet (need lookahead)
                    continue

                # Conformer: mel -> encoded chunks (cache-aware, O(1) per push)
                try:
                    encoded_list = conformer_state.push(mel)
                except Exception as e:
                    print(json.dumps({"error": f"conformer push failed: {e}"}), flush=True)
                    continue

                new_text = False
                for encoded in encoded_list:
                    try:
                        prompted = model.apply_prompt(encoded, language or "auto")
                    except Exception as e:
                        print(json.dumps({"error": f"apply_prompt failed: {e}"}), flush=True)
                        continue

                    chunk_len = int(prompted.shape[1])
                    time_idx = 0
                    new_symbols = 0
                    while time_idx < chunk_len:
                        feature = prompted[:, time_idx : time_idx + 1]
                        current_token = (
                            mx.array([[last_token]], dtype=mx.int32)
                            if last_token != blank_id
                            else None
                        )
                        try:
                            decoder_output, (h, c) = model.decoder(current_token, decoder_hidden)
                        except Exception as e:
                            print(json.dumps({"error": f"decoder failed: {e}"}), flush=True)
                            break
                        decoder_output = decoder_output.astype(feature.dtype)  # type: ignore
                        proposed_hidden = (h.astype(feature.dtype), c.astype(feature.dtype))  # type: ignore
                        try:
                            joint_output = model.joint(feature, decoder_output)
                        except Exception as e:
                            print(json.dumps({"error": f"joint failed: {e}"}), flush=True)
                            break
                        pred_token = int(mx.argmax(joint_output))
                        if pred_token != blank_id:
                            last_token = pred_token
                            decoder_hidden = proposed_hidden
                            if not nemo_tok.is_special_token(last_token, vocabulary):
                                try:
                                    hypothesis.append(
                                        AlignedToken(
                                            last_token,
                                            start=(global_time + time_idx) * frame_sec,
                                            duration=frame_sec,
                                            text=nemo_tok.decode([last_token], vocabulary),
                                        )
                                    )
                                except Exception:
                                    # Fallback without timing
                                    hypothesis.append(last_token)  # type: ignore
                                new_text = True
                            new_symbols += 1
                            if max_symbols is not None and new_symbols >= max_symbols:
                                time_idx += 1
                                new_symbols = 0
                        else:
                            time_idx += 1
                            new_symbols = 0
                    global_time += chunk_len
                    # Keep Metal caches materialized at chunk boundary
                    try:
                        conformer_state.materialize(prompted)
                    except Exception:
                        try:
                            mx.eval(prompted)
                        except Exception:
                            pass

                if new_text:
                    text = _emit_text()
                    print(json.dumps({"committed": text, "tentative": ""}), flush=True)

            elif mtype == "finalize":
                # Flush frontend: emit remaining lookahead frames
                try:
                    mel = frontend.flush()
                    if mel is not None and getattr(mel, "shape", (0, 0))[1] > 0:
                        for encoded in conformer_state.push(mel, final=True):
                            prompted = model.apply_prompt(encoded, language or "auto")
                            chunk_len = int(prompted.shape[1])
                            time_idx = 0
                            new_symbols = 0
                            while time_idx < chunk_len:
                                feature = prompted[:, time_idx : time_idx + 1]
                                current_token = (
                                    mx.array([[last_token]], dtype=mx.int32)
                                    if last_token != blank_id
                                    else None
                                )
                                decoder_output, (h, c) = model.decoder(current_token, decoder_hidden)  # type: ignore
                                decoder_output = decoder_output.astype(feature.dtype)
                                proposed_hidden = (h.astype(feature.dtype), c.astype(feature.dtype))
                                joint_output = model.joint(feature, decoder_output)
                                pred_token = int(mx.argmax(joint_output))
                                if pred_token != blank_id:
                                    last_token = pred_token
                                    decoder_hidden = proposed_hidden
                                    if not nemo_tok.is_special_token(last_token, vocabulary):
                                        hypothesis.append(
                                            AlignedToken(
                                                last_token,
                                                start=(global_time + time_idx) * frame_sec,
                                                duration=frame_sec,
                                                text=nemo_tok.decode([last_token], vocabulary),
                                            )
                                        )
                                    new_symbols += 1
                                    if max_symbols is not None and new_symbols >= max_symbols:
                                        time_idx += 1
                                        new_symbols = 0
                                else:
                                    time_idx += 1
                                    new_symbols = 0
                            global_time += chunk_len
                except Exception as e:
                    print(json.dumps({"error": f"finalize flush failed: {e}"}), flush=True)

                final_text = _emit_text()
                print(json.dumps({"committed": final_text, "tentative": "", "is_final": True}), flush=True)
                break
            elif mtype == "cancel":
                break
            else:
                print(json.dumps({"error": f"unknown type {mtype}"}), flush=True)

        try:
            reset_mlx_memory()
        except Exception:
            pass
        sys.exit(0)

    # ------------------------------------------------------------------
    # Fallback path for Qwen / other mlx-audio streaming models (re-decode)
    # ------------------------------------------------------------------
    runtime = MLXAudioASR(spec)
    # Preload model before signaling readiness
    try:
        runtime.load()
    except Exception as e:
        print(json.dumps({"error": f"model load failed: {e}"}), flush=True)
        sys.exit(1)

    print(json.dumps({"ready": True, "model": spec.alias}), flush=True)
    sys.stderr.write(f"MLX live ready (snapshot): {spec.alias} language={language}\n")
    sys.stderr.flush()

    buffer: list[float] = []
    last_emit_time = time.time()
    last_emit_len = 0
    # Throttle knobs: emulate ~2-3 live updates per second without hammering Metal
    min_interval_s = 0.4
    min_new_samples = 4800  # ~0.3s @16kHz
    max_buffer_samples = 30 * 16000  # 30s window for incremental live; keeps memory bounded

    def transcribe_buffer(buf: list[float]) -> str:
        if not buf:
            return ""
        wav_path = _write_pcm_to_wav(buf)
        try:
            # Re-use the preloaded runtime (avoids reloading model each snapshot)
            # Handles per-alias language mapping internally
            return runtime.transcribe(wav_path, language)
        finally:
            try:
                os.unlink(wav_path)
            except Exception:
                pass
            # Give memory back between snapshots
            try:
                mx.clear_cache()
            except Exception:
                pass

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except Exception as e:
            print(json.dumps({"error": f"bad json: {e}"}), flush=True)
            continue

        mtype = msg.get("type")
        if mtype == "feed":
            samples = msg.get("samples")
            if samples is None:
                # Support alternative key "pcm"
                samples = msg.get("pcm", [])
            # Decode base64 if string
            if isinstance(samples, str):
                try:
                    raw_bytes = base64.b64decode(samples)
                    # Assume float32 LE
                    count = len(raw_bytes) // 4
                    if count:
                        floats = struct.unpack(f"<{count}f", raw_bytes)
                        buffer.extend(floats)
                    else:
                        continue
                except Exception as e:
                    print(json.dumps({"error": f"base64 decode failed: {e}"}), flush=True)
                    continue
            elif isinstance(samples, list):
                # JSON array of floats
                buffer.extend(float(s for s in samples if isinstance(s, (int, float))))
            else:
                print(json.dumps({"error": "feed samples must be list or base64 string"}), flush=True)
                continue

            # Keep buffer bounded to max window (streaming models cap context)
            if len(buffer) > max_buffer_samples:
                # Keep last window; adjust last_emit_len proportionally
                excess = len(buffer) - max_buffer_samples
                buffer = buffer[excess:]
                last_emit_len = max(0, last_emit_len - excess)

            now = time.time()
            new_samples = len(buffer) - last_emit_len
            if new_samples >= min_new_samples and (now - last_emit_time) >= min_interval_s:
                try:
                    text = transcribe_buffer(buffer)
                    # Emit committed; tentative empty (stable prefix model)
                    print(json.dumps({"committed": text, "tentative": ""}), flush=True)
                    last_emit_len = len(buffer)
                    last_emit_time = now
                except Exception as e:
                    print(json.dumps({"error": f"transcribe failed: {e}"}), flush=True)
                    # Keep running; next feed will retry

        elif mtype == "finalize":
            try:
                final_text = transcribe_buffer(buffer)
                print(json.dumps({"committed": final_text, "tentative": "", "is_final": True}), flush=True)
            except Exception as e:
                print(json.dumps({"error": f"finalize failed: {e}"}), flush=True)
            break
        elif mtype == "cancel":
            break
        else:
            print(json.dumps({"error": f"unknown type {mtype}"}), flush=True)

    # Cleanup
    try:
        reset_mlx_memory()
    except Exception:
        pass
    sys.exit(0)


def print_models() -> None:
    print()

    header = (
        f"{'ALIAS':<16}"
        f"{'ENGINE':<12}"
        f"{'GB':>6}  "
        f"{'STREAM':<16}"
        f"{'ACC':>4} "
        f"{'SPD':>4}  "
        f"MODEL"
    )

    print(header)
    print("-" * len(header))

    for spec in MODELS.values():
        print(
            f"{spec.alias:<16}"
            f"{spec.engine:<12}"
            f"{spec.approximate_weight_gb:>6.1f}  "
            f"{spec.streaming:<16}"
            f"{spec.accuracy_score:>4} "
            f"{spec.speed_score:>4}  "
            f"{spec.model_id}"
        )

    print()


# ============================================================================
# CLI
# ============================================================================


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description=(
            "Native Apple MLX speech-to-text "
            "+ local LLM cleanup runtime."
        )
    )

    # Global so callers can place it before or after the subcommand.
    p.add_argument(
        "--json",
        action="store_true",
        help="Emit a single JSON object ({text}) instead of plain text.",
    )

    commands = p.add_subparsers(
        dest="command",
        required=True,
    )

    commands.add_parser(
        "models",
        help="List configured models.",
    )

    recommend = commands.add_parser(
        "recommend",
        help="Choose a model for this Mac.",
    )

    recommend.add_argument(
        "--mode",
        choices=(
            "live",
            "live-multilingual",
            "balanced",
            "quality",
            "diarize",
            "max-accuracy",
        ),
        default="balanced",
    )

    trans = commands.add_parser(
        "transcribe",
    )

    trans.add_argument("audio")

    trans.add_argument(
        "--model",
        default="auto",
        choices=("auto", *MODELS.keys()),
    )

    trans.add_argument(
        "--mode",
        default="balanced",
        choices=(
            "live",
            "live-multilingual",
            "balanced",
            "quality",
            "diarize",
            "max-accuracy",
        ),
    )

    trans.add_argument(
        "--language",
        default=None,
    )

    trans.add_argument(
        "--stream",
        action="store_true",
    )

    clean = commands.add_parser(
        "clean",
    )

    clean.add_argument(
        "text",
    )

    clean.add_argument(
        "--llm",
        default="auto",
    )

    serve = commands.add_parser(
        "llm-serve",
        help=(
            "Long-lived local LLM inference for Rust"
            " (JSONL stdin/stdout, model loaded once)."
        ),
    )

    serve.add_argument(
        "--llm",
        default="auto",
    )

    pipe = commands.add_parser(
        "pipeline",
    )

    pipe.add_argument(
        "audio",
    )

    pipe.add_argument(
        "--model",
        default="auto",
        choices=("auto", *MODELS.keys()),
    )

    pipe.add_argument(
        "--mode",
        default="balanced",
        choices=(
            "live",
            "live-multilingual",
            "balanced",
            "quality",
            "diarize",
            "max-accuracy",
        ),
    )

    pipe.add_argument(
        "--language",
        default=None,
    )

    pipe.add_argument(
        "--llm",
        default="auto",
    )

    live = commands.add_parser(
        "live",
        help="Long-lived incremental streaming for Rust overlay (JSONL stdin/stdout).",
    )

    live.add_argument(
        "--model",
        default="auto",
        choices=("auto", *MODELS.keys()),
    )

    live.add_argument(
        "--mode",
        default="balanced",
        choices=(
            "live",
            "live-multilingual",
            "balanced",
            "quality",
            "diarize",
            "max-accuracy",
        ),
    )

    live.add_argument(
        "--language",
        default=None,
    )

    return p


def main() -> None:
    args = parser().parse_args()

    ram = system_ram_gb()

    if args.command == "models":
        print_models()
        return

    if args.command == "recommend":
        spec = choose_asr(
            args.mode,
            ram,
        )

        print()
        print(f"RAM:       {ram:.1f} GB")
        print(f"Mode:      {args.mode}")
        print(f"Model:     {spec.alias}")
        print(f"HF repo:   {spec.model_id}")
        print(f"Reason:    {spec.notes}")
        print()
        return

    if args.command == "clean":

        llm_id = (
            choose_llm(ram)
            if args.llm == "auto"
            else args.llm
        )

        cleaner = CleanupLLM(llm_id)

        print(
            cleaner.cleanup(args.text)
        )

        return

    if args.command == "llm-serve":

        llm_id = (
            choose_llm(ram)
            if args.llm == "auto"
            else args.llm
        )

        run_llm_serve(llm_id)

        return

    if args.command == "live":
        spec = (
            choose_asr(args.mode, ram)
            if args.model == "auto"
            else MODELS[args.model]
        )
        print(
            f"ASR model: {spec.alias}",
            file=sys.stderr,
        )
        print(
            f"Model ID:  {spec.model_id}",
            file=sys.stderr,
        )
        run_live(spec, args.language)
        return

    spec = (
        choose_asr(args.mode, ram)
        if args.model == "auto"
        else MODELS[args.model]
    )

    print(
        f"ASR model: {spec.alias}",
        file=sys.stderr,
    )

    print(
        f"Model ID:  {spec.model_id}",
        file=sys.stderr,
    )

    if args.command == "transcribe":

        text = transcribe(
            spec=spec,
            audio=args.audio,
            language=args.language,
            stream=args.stream,
        )

        if not args.stream:
            if args.json:
                import json

                print(json.dumps({"text": text}))
            else:
                print(text)

        return

    if args.command == "pipeline":

        raw = transcribe(
            spec=spec,
            audio=args.audio,
            language=args.language,
            stream=False,
        )

        # ASR is no longer needed. Give memory back before loading
        # the cleanup LLM. Very important on unified-memory Macs.
        reset_mlx_memory()

        llm_id = (
            choose_llm(ram)
            if args.llm == "auto"
            else args.llm
        )

        cleaner = CleanupLLM(llm_id)

        cleaned = cleaner.cleanup(raw)

        print()
        print("========== RAW TRANSCRIPT ==========")
        print()
        print(raw)

        print()
        print("========== CLEANED PROMPT ==========")
        print()
        print(cleaned)
        print()

        return


if __name__ == "__main__":
    main()
