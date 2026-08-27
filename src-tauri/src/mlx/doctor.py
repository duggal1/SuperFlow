#!/usr/bin/env python3

from __future__ import annotations

import importlib.metadata
import platform
import subprocess
import sys

import mlx.core as mx


PACKAGES = (
    "mlx",
    "mlx-lm",
    "mlx-audio",
    "parakeet-mlx",
    "transformers",
    "huggingface-hub",
)


def package_version(name: str) -> str:
    try:
        return importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        return "NOT INSTALLED"


def memory_gb() -> float:
    raw = subprocess.check_output(
        ["sysctl", "-n", "hw.memsize"],
        text=True,
    ).strip()

    return int(raw) / (1024**3)


def main() -> None:
    print()
    print("MLX SYSTEM DOCTOR")
    print("=" * 60)

    print(f"Python:        {sys.version.split()[0]}")
    print(f"Executable:    {sys.executable}")
    print(f"Architecture:  {platform.machine()}")
    print(f"macOS:         {platform.mac_ver()[0]}")
    print(f"Unified RAM:   {memory_gb():.1f} GB")
    print(f"Metal:         {mx.metal.is_available()}")

    print()

    for package in PACKAGES:
        print(
            f"{package:<18}"
            f"{package_version(package)}"
        )

    if platform.machine() != "arm64":
        raise SystemExit(
            "\nFAIL: running under Rosetta/x86_64."
        )

    if not mx.metal.is_available():
        raise SystemExit(
            "\nFAIL: MLX cannot access Metal."
        )

    print("\nRunning matrix multiplication...")

    a = mx.random.normal((2048, 2048))
    b = mx.random.normal((2048, 2048))

    c = a @ b
    mx.eval(c)

    peak = mx.get_peak_memory() / 1024**3

    print(f"Result:        {c.shape}")
    print(f"MLX peak RAM:  {peak:.3f} GB")
    print()
    print("PASS: Native MLX/Metal is working.")


if __name__ == "__main__":
    main()
