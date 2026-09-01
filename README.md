# Superflow

**Local-first AI dictation and voice control for macOS.**

[Website](https://superflow.bixbite.fun/)

Superflow turns speech into clean text, understands the app and context you are working in, and can use voice as an interface for real computer workflows.

It started from Handy, but the product has grown far beyond basic speech-to-text.

## What Superflow does

Press a shortcut, speak naturally, and Superflow handles the rest locally.

It supports:

* fast local speech-to-text
* streaming transcription
* context-aware dictation
* deterministic grammar and transcript cleanup
* technical term and code-aware formatting
* selected-text editing
* local AI cleanup
* Gmail and Slack-aware formatting
* application and file context
* voice commands and computer actions
* local LLM support
* developer workflows and terminal automation

Superflow is designed around one idea: dictation should understand where you are and what you are trying to do.

## Local speech recognition

Superflow runs speech recognition directly on your Mac.

The main local ASR path uses Parakeet models through native GGML/Metal inference, including quantized GGUF models optimized for Apple Silicon.

Audio does not need to leave your machine for transcription.

## Context awareness

Superflow can use local macOS context such as:

* active application
* browser or editor surface
* focused text
* selected text
* file names and paths
* technical identifiers
* clipboard and surrounding workflow context

This allows the same spoken sentence to be handled differently depending on where you are working.

A Slack update should look like Slack. An email should look like an email. Developer dictation should preserve things like `useEffect`, `getUserById`, `.env.local`, file paths, commands, and code identifiers.

## Grammar and formatting

Raw speech is messy.

Superflow runs a deterministic post-processing pipeline for things such as:

* repeated words and phrases
* fillers and false starts
* punctuation
* sentence boundaries
* paragraph formatting
* common grammar errors
* technical-name normalization
* spoken mentions and channels

The goal is clean written output without casually rewriting what you meant.

## Local AI

Superflow can also use local LLMs for tasks that need more intelligence than deterministic cleanup.

Local models can handle the same kinds of prompts used by the AI pipeline while keeping inference on the machine.

The deterministic path remains available for fast transcription and formatting without requiring an LLM.

## Voice workflows

Superflow is not limited to inserting text.

Voice commands can be connected to deterministic computer actions and professional workflows.

Examples include:

```text
Open Claude Code and fix the backend.
```

```text
Reply to this email and keep it concise.
```

```text
Rewrite the selected text professionally.
```

The language model, when one is involved, interprets intent. The actual system actions remain controlled by Superflow's native backend.

## Architecture

Superflow is a Tauri application built primarily with:

* Rust
* React
* TypeScript
* MLX
* GGML / Metal
* native macOS Accessibility APIs

The backend handles audio, inference, context capture, deterministic language processing, system integration, and actions.

The frontend stays deliberately small and focused on configuration and control.

## Privacy

Most of Superflow's core functionality can run locally:

* speech recognition
* transcript cleanup
* grammar processing
* context capture
* local LLM inference
* computer actions

Your microphone audio does not need to be uploaded to a transcription service.

## Development

Superflow is under active development and currently focuses primarily on macOS and Apple Silicon.

For build and development instructions, see the repository documentation.

## License

MIT License. See [`LICENSE`](LICENSE).

Superflow is built on top of open-source work including Handy, Tauri, GGML, MLX, Parakeet, and the broader Rust and local-AI ecosystem.
