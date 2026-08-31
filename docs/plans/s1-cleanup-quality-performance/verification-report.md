# S1-mini Cleanup Verification Report

Date: 2026-08-24  
Baseline: `37edaf2` plus the current T4-T8 working-tree implementation  
Release decision: **NO-GO pending live application soak and UI smoke**

## Completed checks

| Check                                    | Result                          | Evidence                                                                                                                              |
| ---------------------------------------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Rust formatting                          | pass                            | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`                                                                           |
| Targeted cleanup tests                   | pass                            | 14/14                                                                                                                                 |
| Full Rust test suite                     | pass                            | 351/351 library tests; 0/0 binary tests                                                                                               |
| Frontend TypeScript/Vite build           | pass                            | 5,668 modules transformed; build completed                                                                                            |
| Frontend ESLint                          | pass                            | `eslint src` returned zero                                                                                                            |
| Git whitespace check                     | pass before final report update | `git diff --check`                                                                                                                    |
| Exact S1 prompt/control/greedy contract  | pass                            | pinned Rust test plus real-GGUF runs                                                                                                  |
| Punctuation-free stable-span scheduling  | pass                            | unit test seals a bounded span and retains tail                                                                                       |
| UTF-8 final-revision reconciliation      | pass                            | JSONL trace replay covers monotonic, middle, self-correction, Unicode, and language-skip cases                                        |
| Exact boundary whitespace                | pass                            | unit test                                                                                                                             |
| Common-word catalog collision regression | pass                            | full Rust suite includes prose/technical catalog tests; quality fixture includes `for an`                                             |
| Real M1 concurrency selection            | pass                            | two slots rejected; one model/context selected                                                                                        |
| Real M1 quality sanity check             | pass                            | raw lowercase ASR normalized without random uppercase; pre-corrupted uppercase remained uppercase, confirming T2 ordering is required |
| Accelerated 30-minute model replay       | pass with evidence boundary     | 22 background spans; 60-word tail completed in 1.122 s; protected invariants passed                                                   |
| Developer-context routing                | pass                            | terminal/editor prompts now use bounded visible, project, branch, tracked-change, and repository-instruction metadata                 |
| Local developer fallback                 | pass                            | context packaging produces a faithful Claude Code-ready prompt without requiring a cloud model                                        |

## Known repository-wide gate failures

`cargo clippy --all-targets -- -D warnings` is not green. After fixing the two
new cleanup warnings, the command still reports pre-existing warnings in
`actions.rs`, `lib.rs`, recorder/history/GGUF metadata, transcription,
portable mode, and voice-terminal modules. These files/classes are outside the
cleanup change and were not broadly refactored.

`bun run check:translations` is also not green. Every one of the 23 non-English
locale files already lacks approximately 40 English reference keys. New
Cleaning/Finalizing/Degraded labels use explicit English `defaultValue`
fallbacks and do not add new reference keys, so this change does not increase
that drift.

## Mandatory checks still unrun

- 30-minute real microphone capture with Parakeet and S1 running concurrently;
- ASR real-time-factor before/after comparison under Metal cleanup load;
- process RSS, macOS memory-pressure, and swap capture during that soak;
- Tauri overlay/Journal smoke using installed model events;
- rapid record/cancel/re-record interaction on the packaged application;
- three or more 10-minute and 30-minute stop-tail samples for p95 claims;
- blind human A/B quality preference scoring over the complete corpus.

These require the interactive app, microphone input, and human review. They are
not replaced by unit tests or the accelerated GGUF replay.

## Release criteria

Change to `GO` only after the unrun checks show:

- no lost, duplicated, reordered, or stale text;
- no unexplained common-word ALL-CAPS output;
- no protected number, currency, percentage, path, identifier, or negation loss;
- 10-minute stop-to-paste <= 1.5 s p50 and <= 3.0 s p95;
- 30-minute stop-to-paste <= 2.5 s p50 and <= 5.0 s p95;
- ASR real-time-factor regression <= 10%;
- no swap, red memory pressure, crash, deadlock, or stuck UI;
- truthful Cleaning, Finalizing, Active, Degraded, Loading, and Unavailable UI;
- blind A/B preference >= 80% with zero hard-invariant failures.
