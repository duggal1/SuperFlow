# Superflow

**Free, open-source AI dictation with deep context awareness for macOS.**

[Website](https://superflow.bixbite.fun/)

Superflow turns speech into clean text, understands what is on your screen, knows the app you are working in, and uses that context to produce better output.

Today, Superflow is focused on extremely fast local dictation, context-aware writing, meeting intelligence, and practical voice workflows.

The longer-term goal is bigger: combine speech-to-text with a powerful context engine so Superflow can eventually understand your computer well enough to do work for you, not just type for you.

If the current release gets strong feedback, the next phase will push much further into actions like scheduling meetings, filling forms, booking demos, handling repetitive business workflows, and controlling more of the computer through voice.

## Context-aware dictation

Superflow does more than transcribe words.

It can understand the app, screen, selected text, focused content, files, technical identifiers, and surrounding context you are working with.

If you are replying inside Gmail, Superflow can use the email context around your reply.

If you are writing in Slack, it can understand that you are writing a Slack message and format the output accordingly.

If you are working in code, it can preserve and normalize technical language such as:

```text
useEffect
getUserById
.env.local
SuperflowPanel
src-tauri/src/audio_toolkit/transcript_cleanup.rs
```

The same spoken sentence can produce different output depending on where you are working.

That is the point of Superflow: transcription should understand the place where the words are going.

## Fast local speech recognition

Superflow runs speech recognition locally on your Mac.

We strongly recommend using a streaming model.

Streaming models work far better for long dictation because they process audio continuously instead of waiting for the entire recording to finish.

Superflow supports multiple local streaming models, with Parakeet 0.6B being the model we currently recommend for most users.

It is small, fast, and surprisingly strong for its size.

On an M1 Mac, real-world transcription can run around 5x to 10x faster than real time depending on the workload. Newer Apple Silicon machines can go significantly faster, with stronger M-series hardware reaching much higher throughput.

This becomes especially useful for long recordings.

You can record minutes or hours of speech and avoid the painful wait associated with large non-streaming transcription jobs.

If you do not like seeing partial words appear live, you can disable the live streaming preview in the frontend. Superflow can still use the streaming model underneath, so you keep the speed without watching the transcript build word by word.

## Meeting intelligence

Superflow can record and process long meetings using the same streaming transcription system.

Because transcription is processed continuously, even very long recordings can finalize extremely quickly once recording ends. In our current usage, finalization is typically only a few seconds rather than forcing you to wait through the entire recording again.

After transcription, Superflow can turn the meeting into a useful report instead of leaving you with a giant wall of text.

You can use it to understand things like:

* what happened in the meeting
* important decisions
* action items
* what went well
* what could be improved
* important things you may have missed
* follow-up work
* questions about the meeting

You can also ask questions directly against the meeting transcript and use the full recording as context.

The idea is simple: record the meeting once, then actually get something useful from it.

## Calendar and meeting workflows

Superflow is also moving beyond transcription into voice-driven workflows.

You can use voice to create meeting actions instead of manually jumping between apps.

For example:

```text
Schedule a meeting with Sarah tomorrow at 3 PM.
```

Superflow can interpret the request, resolve the action, and create the event through your calendar workflow.

The same architecture can expand into checking meetings, preparing context before a meeting, generating follow-ups afterward, and eventually handling much more of the surrounding work.

## Grammar and cleanup

Raw speech is messy. People repeat themselves, restart sentences, change their mind halfway through a phrase, forget punctuation, and generally commit crimes against written English while speaking perfectly normally.

Superflow cleans that up automatically.

The post-processing pipeline handles things such as:

* repeated words and phrases
* filler words
* false starts
* grammar
* punctuation
* sentence boundaries
* paragraph formatting
* technical terminology
* spoken mentions and channels
* common speech-to-text mistakes

The goal is clean written text while keeping the meaning intact.

For fast everyday transcription, most of this work happens without needing a large language model.

## Local AI

For tasks that need more reasoning, Superflow can also use local LLMs.

This lets you go beyond transcription and ask the system to rewrite, summarize, structure, interpret, or work with the context already available on your machine.

Examples:

```text
Rewrite this professionally.
```

```text
Reply to this email and keep it concise.
```

```text
Summarize this meeting and give me the action items.
```

```text
Fix the selected text without changing what I mean.
```

Local models can run directly on Apple Silicon, keeping the workflow fast and private without making cloud AI mandatory.

## Voice workflows

Superflow is gradually turning speech into an interface for the computer itself.

Today, that includes dictation, editing, context-aware output, meeting workflows, and developer actions.

The next step is letting voice trigger much larger workflows.

Examples of where this is going:

```text
Open Claude Code and fix the backend.
```

```text
Schedule a meeting with Alex for Friday afternoon.
```

```text
Fill this form using my current information.
```

```text
Book a product demo for next week.
```

```text
Check my meetings for tomorrow and prepare me for them.
```

Instead of giving an AI unrestricted control over the machine, Superflow can use AI to understand the request and then pass the result into controlled native actions.

That keeps the system fast, predictable, and much easier to trust.

## Built for real work

Superflow is aimed at people who spend serious time writing, coding, replying, documenting, meeting, and operating through a computer.

The useful part is not simply converting audio into text.

It is having enough context to know:

* where the text is going
* what you are working on
* what information already exists on screen
* what application you are inside
* whether you are writing an email, Slack message, document, or code-related prompt
* what action you are actually trying to perform

As the context engine grows, the distinction between "dictation" and "voice control" becomes smaller.

## Architecture

Superflow is a Tauri application built primarily with:

* Rust
* React
* TypeScript
* MLX
* GGML / Metal
* native macOS APIs

The Rust backend handles audio, speech recognition, local inference, grammar processing, context capture, application awareness, and system actions.

The frontend stays intentionally small and focuses on settings, models, controls, and the information you actually need.

## Privacy

Superflow is designed around local execution.

Core functionality can run directly on your machine, including:

* speech recognition
* transcript cleanup
* grammar processing
* context capture
* meeting transcription
* local LLM inference
* computer actions

Your microphone audio does not need to be sent to a transcription service.

## What's next

The current release is focused on getting the core experience right first: fast transcription, strong context awareness, reliable formatting, meetings, and local AI.

If users find that foundation useful, Superflow will expand much further into computer control.

The direction is:

**speech-to-text + context engine + controlled actions**

That means scheduling meetings, working with email, filling forms, handling business workflows, launching developer tools, understanding what is on screen, and eventually letting you operate much more of your computer through voice.

Superflow should not stop at being a better dictation app.

The goal is to make speech a serious interface for getting work done.

## Development

Superflow is under active development and currently focuses primarily on macOS and Apple Silicon.

For build and development instructions, see the repository documentation.

## License

MIT License. See [`LICENSE`](LICENSE).

Superflow is built with open-source technologies including Tauri, Rust, MLX, GGML, Parakeet, and the broader local-AI ecosystem.
