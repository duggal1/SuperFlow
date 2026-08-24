# S1-mini Cleanup Quality and Streaming Performance Plan

Status: implementation-ready plan, not implementation  
Target: SuperFLow macOS on Apple M1 MacBook Air, 8 GB unified memory  
Task source of truth: [`tasks.json`](./tasks.json)

## 1. Objective

Make S1-mini the single owner of English transcript grammar, punctuation,
capitalization, filler removal, self-correction, paragraphs, and conservative
lists. Preserve only narrow, meaning-safe deterministic token normalization.
Start cleanup from stable ASR commits while the user is still recording so a
long dictation does not pay the full autoregressive generation cost after stop.
Every model rejection, failure, fallback, queue delay, and final text source
must be observable and testable.

This is a local Rust/Tauri backend change with a small truthful-state frontend
update. It is not a remote backend project and it is not a UI redesign.

## 2. Definition of done

The work is done only when all of these are true:

1. Ordinary prose cannot be converted into unexplained `TODAY`, `AND`,
   `RIGHT`, `NOW`, or similar programming/spreadsheet tokens.
2. No deterministic production path adds English sentence punctuation,
   capitalization, paragraphs, headings, or lists before or after S1-mini.
3. S1-mini uses the exact trained system prompt, empty think block, greedy
   decoding, and valid control line:

   ```text
   [Styling: formal] [Structure: lists] [Context: general]
   ```

4. Streaming-capable English ASR feeds only stable committed text to a
   recording-scoped cleanup session while capture is active.
5. At stop, normally only one unresolved chunk plus a revision tail remains.
6. Chunk results are sequence-numbered, validated independently, assembled in
   source order, and fall back independently. One rejected chunk cannot throw
   away every successful chunk.
7. Cleanup remains forced to Apple Metal regardless of the speech model's CPU,
   Auto, or GPU selection.
8. Concurrency is bounded and selected from measurements on this M1. The model
   weights are loaded once. No configuration that swaps, starves ASR, or
   worsens end-to-end latency ships.
9. The backend reports whether final text came from S1, mixed chunk fallback,
   raw fallback, or a language skip. The frontend never labels a rejected run
   as successful cleanup.
10. The quality, latency, memory, soak, failure-injection, Rust, TypeScript,
    lint, format, and translation gates in Task T8 pass.

## 3. What the model can and cannot do

### Supported and in scope

S1-mini is a task-specific English text normalizer. Its documented and tested
job is to:

- remove filled pauses;
- resolve false starts and self-corrections;
- apply punctuation and sentence casing;
- normalize spoken numbers, dates, times, currency, and email addresses;
- produce prose or conservative Markdown bullet lists when the source contains
  a real enumeration of at least roughly three items.

### Unsupported and explicitly not promised

S1-mini is not a general instruction-following LLM. It cannot be prompted into
reliable rich Markdown headings, bold emphasis, links, summaries, arbitrary
sections, or a custom document schema. `font-normal`, `markdown`, `bullets`,
and `work` are not trained control values. Sending those values is invalid and
can produce ignored, garbled, or hallucinated output.

The requested phrase “full font-normal instead of semi-formal” is therefore
implemented as the valid model control `Styling: formal`. That setting affects
language register; it has nothing to do with CSS font weight. `Structure:
lists` remains the strongest supported Markdown-like structure.

If rich Markdown is required later, it is a separate explicit feature using a
real instruction-following model. It is not part of this plan because it would
add latency and change the product contract.

## 4. Confirmed current-state findings

These are repository and runtime facts, not guesses.

| Finding | Evidence | Consequence |
|---|---|---|
| S1 is installed, loaded, and used | `superflow.log` records the model loaded with 99 Metal layers and multiple completed cleanup jobs | The primary failure is not “the model never loads.” |
| Some outputs are silently discarded | The same log records `output failed fidelity validation; failing open` | A user can receive pre-S1 text after waiting for S1 generation and cannot tell that happened. |
| A real 2,584-character job took 8.46 seconds and was discarded | Runtime log entry immediately pairs rejection with the 8.46-second completion | Current architecture pays latency and receives no quality benefit on rejected runs. |
| Long text is processed only after final ASR text exists | `actions.rs` calls `local_cleanup::normalize` after transcription finalization | A 10–30 minute recording necessarily accumulates a large stop-time cleanup bill. |
| Current cleanup worker is serial | `local_cleanup.rs` owns one receiver, one context, and a sequential loop over chunks | Chunk count increases wall time linearly at stop. |
| Current chunking is estimated by 500 whitespace words | `MAX_CHUNK_WORDS` and `chunk_transcript` | It is not bounded by the model tokenizer and raw ASR without punctuation often hard-splits at arbitrary word positions. |
| One invalid chunk rejects the complete job | `failed = true` returns `None` for the whole request | Good S1 chunks are lost and raw pre-S1 text becomes final. |
| Global programming syntax rewrites plain English | `programming-syntax.json` contains `and → AND`, `right → RIGHT`, `today → TODAY`, and `now → NOW`; the harvester applies them globally | The exact reported random uppercase output has a deterministic upstream cause. |
| `get request → GET` is phrase-scoped | The catalog does not define bare `get → GET` in that API entry | `GET` still requires collision testing, but it is not the same confirmed bare-word alias as `AND` or `TODAY`. |
| The speech accelerator is independent from S1 | Speech logs alternate between CPU and `MTL0`; S1 startup remains 99 Metal layers | Keep the independence and add a regression test; do not build a second cleanup accelerator dropdown. |
| The UI already derives active from installed plus ready | `CleanupModelStatus.active` and `AIModelsStatusCard.tsx` | Extend real state; do not replace it with a static label. |
| `llama-cpp-2` supports sequence-aware scheduling primitives | Installed 0.1.154 exposes `n_seq_max`, batch sequence IDs, KV sequence operations, `n_batch`, and `n_ubatch` | Multi-sequence batching is feasible to benchmark, not automatically correct to ship. |

## 5. Root-cause model

The observed bad output has four independent causes. Prompt tuning alone cannot
fix them.

### 5.1 Unsafe deterministic input mutation

The current manager applies the technical lexicon, styling catalog, and entire
programming-syntax catalog to every transcript before S1. Spreadsheet and SQL
aliases are harvested as global token substitutions. Ordinary speech therefore
enters S1 already containing uppercase syntax tokens.

Example current path:

```text
raw ASR:   "what I did today and right now"
catalog:   "what I did TODAY AND RIGHT NOW"
S1 input:  already corrupted casing
```

The old deterministic formatter had a de-shout pass that happened to hide some
of this. Removing that formatter exposed the catalog defect. Reintroducing the
formatter would only mask the source and recreate competing grammar logic.

### 5.2 Opaque all-or-nothing fail-open

S1 can run successfully and then fail local validation. The caller receives
`None`, keeps the earlier text, and the UI has no run-level outcome. This makes
“model was used” and “model output became final” indistinguishable.

### 5.3 Stop-time serial autoregressive generation

Approximately 1,000 tokens/second is prompt evaluation, not output generation.
The measured output generation rate is around 80–95 tokens/second on this
machine. A long cleaned transcript must generate a long output, so a fully
serial request after stop cannot meet a one-second target. The principal fix is
to do the work during recording; concurrency is secondary.

### 5.4 Chunking without streaming or source-span ownership

Current chunks are created after stop, measured in words, processed
sequentially, joined with forced blank lines, and validated as one job. There
is no session identity, committed-prefix tracking, reorder buffer, revision
tail, chunk-local fallback, backpressure, or cancellation ownership.

## 6. Architecture decision

### 6.1 Target flow

```text
microphone audio
  -> streaming Parakeet ASR
  -> committed prefix updates -----------------------------+
  -> tentative text remains UI-only                        |
                                                           v
final ASR snapshot -> cleanup session reconciler -> stable segment assembler
                                                   -> bounded Metal scheduler
                                                   -> S1 validation per chunk
                                                   -> ordered chunk assembler
                                                   -> safe token normalization
                                                   -> optional existing features
                                                   -> paste and history
```

### 6.2 Stage ownership

| Stage | Owns | Must not own |
|---|---|---|
| ASR | speech decoding and committed/tentative hypotheses | grammar cleanup or Markdown |
| S1-mini | English fillers, self-correction, grammar, punctuation, casing, paragraphs, conservative lists | rich Markdown or arbitrary instructions |
| Post-S1 token normalizer | explicit technical terms, paths, extensions, exact values, and high-confidence canonical tokens | prose grammar, sentence casing, punctuation, paragraphing, headings, or list inference |
| Validator | meaning-preservation invariants and precise rejection reasons | rewriting the candidate to make it pass |
| Assembler | sequence ordering and original boundary whitespace | grammar or content transformation |
| Frontend | truthful lifecycle and outcome rendering | guessing backend state |

### 6.3 Cleanup session contract

The implementation should converge on this internal contract, adapted to the
project's exact visibility conventions:

```rust
begin_session(session_id, effective_language)
push_committed(session_id, revision, committed_snapshot)
finish_session(session_id, final_text) -> CleanupOutcome
cancel_session(session_id)
```

Rules:

- `committed_snapshot` is accepted only when its revision is newer and its text
  is a monotonic extension of the prior committed source.
- tentative text never enters S1;
- stable chunks receive sequence numbers and exact source spans;
- a 40–80 token provisional tail is retained until a later boundary or final
  snapshot so self-corrections are not sealed prematurely;
- finalization invalidates only the first divergent provisional span;
- all jobs carry the session ID and cancellation generation;
- late results from a cancelled or superseded session are ignored;
- output is pasted only from one terminal `CleanupOutcome`.

### 6.4 Outcome contract

Replace `Option<String>` with an exhaustive result:

```text
Applied
PartiallyApplied
Skipped(non-English or empty)
Rejected(validation reason)
Failed(engine, timeout, queue, or lifecycle reason)
Cancelled
```

Every terminal outcome includes a privacy-safe metric summary and a final
source classification:

```text
fully_s1 | mixed_chunk_fallback | raw_fallback | non_english_skip
```

### 6.5 Concurrency decision

Do not implement “massive parallelism.” On an 8 GB unified-memory M1, GPU,
memory, KV cache, and streaming ASR all share constrained resources. More
workers can reduce throughput through Metal contention or create swap.

Benchmark these candidates with one loaded model:

1. one reusable context, serial generation;
2. two contexts sharing one model;
3. one context with two active sequences and batched decode;
4. only if the earlier candidates remain healthy, concurrency three and four.

Ship the smallest candidate that improves aggregate cleanup throughput by at
least 20%, improves stop-to-paste p95 by at least 15%, preserves byte-identical
greedy output or equivalent semantics, adds no swap, and worsens streaming ASR
real-time factor by no more than 10%. If none passes, ship one incremental
worker. Starting earlier provides the dominant latency win.

## 7. Chunking and boundary policy

### 7.1 Use model tokens, not words

`MAX_CHUNK_WORDS` must disappear from production. Tokenize with the loaded S1
tokenizer and include:

- exact system and control prompt overhead;
- input tokens;
- output budget `1.3 * input_tokens + 32`;
- end-of-generation and safety headroom;
- the configured `N_CTX`.

No chunk may be enqueued unless the full worst-case budget fits.

### 7.2 Boundary priority

Close a stable chunk at the strongest available boundary below the token cap:

1. explicit ASR paragraph break;
2. sentence terminal;
3. strong clause boundary plus a committed pause or later stable text;
4. hard token boundary only as the last resort.

Hard token splits preserve source whitespace metadata and never imply a blank
paragraph. `results.join("\n\n")` is not allowed as a generic assembly rule.

### 7.3 Revision tail

Keep a provisional tail rather than finalizing every commit immediately. This
protects:

- “Friday, no, wait, Thursday” self-corrections;
- enumerations whose third item arrives later;
- clause punctuation that depends on the next committed phrase;
- ASR finalization differences.

The exact tail size is tuned in T5/T8; the initial bounded search is 40, 60,
and 80 model tokens.

## 8. Safe normalization policy

### 8.1 Remove from the English production path

- deterministic sentence punctuation;
- deterministic sentence or title casing;
- de-shout as a repair for catalog mistakes;
- deterministic paragraph or Markdown-list inference;
- global application of every styling and programming alias;
- filler removal that competes with S1;
- any model prompt containing the JSON catalogs.

### 8.2 Keep, but narrow and move after S1

- user-authored exact custom words;
- canonical brand and framework spellings with unambiguous aliases;
- explicit file extensions and path separators;
- syntax-shaped tokens such as `DATABASE_URL`;
- numeric, currency, percentage, unit, time, and date normalization where it
  preserves explicit source values and does not conflict with S1 output;
- exact whitespace and repeated-token hygiene that cannot change meaning.

### 8.3 Context-gate ambiguous technical aliases

Bare common English words default to prose. A technical rewrite requires
strong local evidence.

| Source phrase | Expected |
|---|---|
| `what I did today` | `what I did today` before S1 casing |
| `and right now` | `and right now` |
| `send a GET request` | preserve or normalize `GET` |
| `use a RIGHT JOIN` | preserve or normalize `RIGHT JOIN` |
| `use SQL A AND B` | preserve SQL `AND` only inside the explicit SQL span |
| `call TODAY open paren` | normalize `TODAY()` only with formula evidence |
| `set database URL env var` | normalize `DATABASE_URL` |
| `use bg stone 700` | normalize `bg-stone-700` |

This policy must be encoded in tests, not left as a comment.

## 9. Validation policy

Validate each model chunk against only its source span. Validation is a guard,
not another formatter.

Required rejection reasons:

- think-tag or template leakage;
- repetition loop;
- invented file, path, extension, identifier, or code block;
- missing explicit numeric token;
- missing currency or percentage;
- missing or reversed negation;
- implausible truncation;
- empty output for meaningful speech;
- generation error or timeout;
- session cancellation or supersession.

Valid empty output for filler-only/noise-only speech remains supported.

When one chunk fails:

1. record the reason;
2. substitute that chunk's safe source fallback;
3. keep accepted neighbor chunks;
4. mark the run `PartiallyApplied` and final source
   `mixed_chunk_fallback`;
5. surface degraded state through the existing backend status and UI.

## 10. Performance measurement contract

### 10.1 Metrics

Record, per run and per chunk:

- recording duration;
- raw character, word, and tokenizer counts;
- queue depth and queue wait;
- prompt evaluation tokens/second and milliseconds;
- output generation tokens/second and milliseconds;
- validation milliseconds and reason;
- assembly milliseconds;
- completed background chunks at stop;
- unresolved tokens and chunks at stop;
- stop-to-paste latency;
- ASR compute time and real-time factor;
- process RSS, macOS memory pressure, and swap;
- backend (`Metal`) and active sequence/context count;
- terminal outcome and final source.

No transcript content is logged.

### 10.2 Fixture durations

Measure:

- short: 30–60 seconds;
- medium: 2–5 minutes;
- long: 10 minutes;
- very long: 20 and 30 minutes;
- overload: synthetic speech rate faster than cleanup throughput.

### 10.3 Latency gates

For normal streaming recordings where cleanup runs throughout capture:

| Duration | Stop-to-paste p50 | Stop-to-paste p95 |
|---|---:|---:|
| 10 minutes | <= 1.5 s | <= 3.0 s |
| 30 minutes | <= 2.5 s | <= 5.0 s |

Additional gates:

- at stop, backlog is no more than one normal chunk plus the retained tail;
- no ASR real-time-factor regression greater than 10%;
- no swap or red memory pressure;
- no crash, deadlock, dropped source span, stale session result, or queue loss;
- overload is reported honestly and remains bounded through backpressure.

These are release gates, not claims about the current implementation.

## 11. Quality evaluation contract

Build a sanitized corpus from real failure classes, not synthetic grammar-only
sentences.

### Required categories

- conversational prose with profanity and colloquialisms;
- `TODAY`/`AND`/`RIGHT`/`NOW` collision family;
- repeated words and repeated clauses;
- false starts and self-corrections;
- explicit numbers, decimals, percentages, currency, dates, and times;
- negations and scope words such as `not`, `never`, `without`, and `only`;
- TypeScript, Rust, paths, file extensions, Tailwind classes, environment
  variables, and API phrases;
- true three-plus-item enumerations;
- prose containing “first” or “right” that is not a list or technical token;
- filler-only/noise-only input;
- English plus non-English skip cases;
- chunk-boundary and final-revision cases.

### Hard invariants

Require 100% preservation of:

- explicit digits and values;
- currency and percentages;
- paths, extensions, identifiers, and canonical technical tokens;
- negations;
- source ordering;
- no invented headings, code blocks, paths, extensions, or tasks;
- no repetition loops;
- no unexplained common-word ALL-CAPS output.

### Human quality gate

Run blind A/B review against the current release. The new result must be
preferred in at least 80% of cases, with no hard-invariant failure. Score:

- faithful meaning;
- grammar and punctuation;
- sentence casing;
- filler and false-start cleanup;
- natural paragraphing;
- list appropriateness;
- voice preservation;
- absence of bloat or invented content.

The published 94.8% token accuracy is model evidence, not a substitute for
this application's corpus.

## 12. Frontend behavior

Keep the existing restrained UI. Do not redesign the Journal card.

### Journal card

- `Active`: model installed and engine ready;
- `Loading`: installed, engine not ready, no terminal load error;
- `Installing`: download or verification in progress;
- `Unavailable`: missing or engine error;
- `Degraded`: last English cleanup completed with mixed or raw fallback.

The status ping and badge continue to use the existing green, orange, and rose
semantics. The backend supplies the state.

### Recording overlay

- raw committed and tentative ASR remain the live visible transcript;
- background cleanup does not rewrite text under the user's eyes;
- `Cleaning` appears only when committed cleanup has a real backlog;
- `Finalizing` appears only while unresolved tail work remains after stop;
- if background cleanup is caught up, proceed directly to paste;
- cancellation removes every pending cleanup state and cannot leak a late
  result into the next recording.

## 13. Ordered implementation plan

| Order | Task | Serves | Depends on | Estimated LOC |
|---:|---|---|---|---:|
| 1 | T1 — Make every cleanup stage and fallback measurable | c1, c3, c7 | — | 360 |
| 2 | T2 — Remove deterministic prose rewriting and unsafe global catalog aliases | c2, c3, c6 | T1 | 390 |
| 3 | T3 — Make the S1 contract exact and fallback chunk-local | c1, c3, c6 | T1 | 350 |
| 4 | T4 — Clean stable ASR commits during recording | c3, c4, c6 | T2, T3 | 400 |
| 5 | T5 — Benchmark bounded llama.cpp scheduling on the target M1 | c1, c5, c8 | T1, T3 | 300 |
| 6 | T6 — Integrate the measured Metal scheduler with backpressure | c4, c5, c6 | T4, T5 | 400 |
| 7 | T7 — Finalize ordered text and expose truthful cleanup progress | c6, c7 | T4, T6 | 380 |
| 8 | T8 — Prove quality, latency, memory, and rollout safety | all | T2, T3, T6, T7 | 300 |

Each task's exact files, types, atomic subtasks, tests, acceptance criteria, and
suggested execution skills are defined in [`tasks.json`](./tasks.json).

## 14. PR execution rules

1. Execute tasks strictly in the listed topological order.
2. T1 must land before behavioral fixes so every later result can be proven.
3. T2 and T3 may be developed in parallel after T1, but each must pass its own
   regression suite before merge.
4. T5 is an experiment with a written decision. T6 may implement only the T5
   winner; it may not pick a concurrency design by intuition.
5. Each PR must run its targeted tests and the strongest relevant subset of the
   final commands.
6. Do not combine failures into a broad refactor. Close one root-cause class per
   PR and keep previously passing fixtures green.
7. Do not claim latency improvement without before/after data from identical
   fixture hashes and hardware.
8. Do not store raw personal transcripts in logs, fixtures, reports, or commits.

## 15. Risks and mitigations

| Risk | Failure mode | Mitigation |
|---|---|---|
| Streaming cleanup seals text too early | Later self-correction becomes inconsistent | Retained provisional tail plus final divergence reconciliation |
| Lists span chunks | First and second items become prose before third arrives | Keep enumeration-bearing tail provisional and test three-item boundary traces |
| Parallel Metal decode starves ASR | Live transcript lags or drops commits | Benchmark with live Parakeet, hard cap concurrency, ASR RTF gate |
| Extra contexts exhaust unified memory | Swap and severe latency spikes | One model load, measure KV memory, reject red pressure or any swap |
| Validation rejects legitimate cleanup | User waits and receives raw text | Precise reasons, chunk-local fallback, corpus-driven threshold changes |
| Normalizer still corrupts prose | ALL-CAPS or code tokens survive after S1 | Post-S1 safe tier, technical span gating, deny ambiguous global aliases |
| Final ASR snapshot diverges from commits | Missing, duplicated, or stale chunks | Source spans, revision IDs, first-divergence invalidation, ordered assembler |
| Engine failure leaves pending callers | Frozen overlay or stuck paste | Fail every waiter exactly once, READY false, failure event, recovery test |
| Formal style changes voice too much | Clean but unnatural output | Blind quality corpus; any meaning or voice regression blocks release |
| S1 cannot create desired rich Markdown | Product expectation remains impossible | Explicit scope: conservative lists only; separate real LLM required later |

## 16. Rollout and rollback

During implementation, retain one internal compatibility switch between the
current whole-transcript path and the new session path. It is not user-facing.

Rollout sequence:

1. ship observability with current behavior;
2. ship safe normalization and chunk-local outcomes;
3. enable incremental cleanup for development builds;
4. run 30-minute soak and failure injection on the target M1;
5. enable for release only after T8 is a documented `go`;
6. remove temporary compatibility plumbing after the confidence window.

Rollback immediately if any of these occurs:

- lost or reordered source text;
- invented technical token, path, extension, number, or negation change;
- stale output from another recording;
- deadlock, stuck overlay, or crash;
- swap or red memory pressure;
- ASR real-time-factor regression above 10%;
- stop-to-paste p95 misses the defined gate;
- common-word ALL-CAPS regression returns.

Rollback restores the prior scheduling path only. It must not restore unsafe
global aliases or deterministic grammar/punctuation rewriting.

## 17. Final verification commands

Run from the repository root:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib
bun run build
bun run lint
bun run format:check
bun run check:translations
```

Then run the production-faithful S1 benchmark, long stream replays, concurrent
Parakeet/S1 soak, failure injection, and Tauri UI smoke cases defined in T5 and
T8. Record exact commands and results in `benchmark-results.md` and
`verification-report.md`. An unrun check must be labeled unrun; a failed check
keeps the release at no-go.

## 18. Reference material

- S1-mini model contract: <https://huggingface.co/superwhisper/s1-mini>
- S1-mini GGUF distribution: <https://huggingface.co/superwhisper/s1-mini-GGUF>
- Installed runtime APIs: `llama-cpp-2` 0.1.154 and `transcribe-cpp` 0.2.0 in
  `src-tauri/Cargo.lock`
- Current cleanup engine: `src-tauri/src/local_cleanup.rs`
- Current stream integration: `src-tauri/src/managers/transcription.rs`
- Current output orchestration: `src-tauri/src/actions.rs`
- Unsafe catalog source: `src-tauri/src/catalog/programming-syntax.json`
- Current model status UI: `src/components/settings/general/AIModelsStatusCard.tsx`

## 19. How to execute

Hand [`tasks.json`](./tasks.json) to the implementation loop and execute tasks
T1 through T8 in `order`. Do not begin T6 until T5 records a benchmark winner.
