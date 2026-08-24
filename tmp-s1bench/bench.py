"""Reproducible S1-mini latency and bounded-concurrency probe."""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import statistics
import time
import urllib.request
from pathlib import Path

SYSTEM_PROMPT = (
    "You are a text normalizer for speech-to-text transcripts. The input begins "
    "with a control line specifying the styling, structure, and context settings; "
    "clean the transcript to match those settings and output only the cleaned text."
)
CONTROL_LINE = "[Styling: formal] [Structure: lists] [Context: general]"
DEFAULT_TRANSCRIPT = (
    "okay so um here is what we need to do first finalize the pricing page at "
    "twenty nine dollars a month second maria records the demo by thursday third "
    "email beta users and fix the export bug before friday because it is blocking "
    "the launch"
)


def build_prompt(transcript: str) -> str:
    return (
        f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
        f"<|im_start|>user\n{CONTROL_LINE}\n{transcript}<|im_end|>\n"
        "<|im_start|>assistant\n<think>\n\n</think>\n\n"
    )


def request_once(url: str, prompt: str) -> dict[str, object]:
    body = json.dumps(
        {"prompt": prompt, "n_predict": 2_000, "temperature": 0}
    ).encode()
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
    )
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=120) as response:
        payload = json.loads(response.read())
    timings = payload["timings"]
    return {
        "wall_seconds": time.perf_counter() - started,
        "prompt_tokens_per_second": timings["prompt_per_second"],
        "generated_tokens_per_second": timings["predicted_per_second"],
        "generated_tokens": timings["predicted_n"],
        "content": payload["content"],
    }


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = min(len(ordered) - 1, round((len(ordered) - 1) * fraction))
    return ordered[index]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:8913/completion")
    parser.add_argument("--parallel", type=int, default=1)
    parser.add_argument("--repeat", type=int, default=10)
    parser.add_argument("--text-file", type=Path)
    arguments = parser.parse_args()

    transcript = (
        arguments.text_file.read_text().strip()
        if arguments.text_file
        else DEFAULT_TRANSCRIPT
    )
    prompt = build_prompt(transcript)
    request_once(arguments.url, prompt)

    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(
        max_workers=arguments.parallel
    ) as executor:
        samples = list(
            executor.map(
                lambda _: request_once(arguments.url, prompt),
                range(arguments.repeat),
            )
        )
    elapsed = time.perf_counter() - started
    walls = [float(sample["wall_seconds"]) for sample in samples]
    generated = [
        float(sample["generated_tokens_per_second"]) for sample in samples
    ]
    outputs = {str(sample["content"]) for sample in samples}
    total_tokens = sum(int(sample["generated_tokens"]) for sample in samples)

    print(
        json.dumps(
            {
                "parallel": arguments.parallel,
                "requests": arguments.repeat,
                "batch_wall_seconds": round(elapsed, 3),
                "request_wall_p50": round(statistics.median(walls), 3),
                "request_wall_p95": round(percentile(walls, 0.95), 3),
                "request_wall_p99": round(percentile(walls, 0.99), 3),
                "generation_tps_p50": round(statistics.median(generated), 1),
                "aggregate_generation_tps": round(total_tokens / elapsed, 1),
                "outputs_identical": len(outputs) == 1,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
