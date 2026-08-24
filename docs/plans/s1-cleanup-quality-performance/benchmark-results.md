# S1-mini Metal Scheduler Benchmark

Date: 2026-08-24  
Machine: Apple M1 MacBook Air, 8 GB unified memory  
Model: `s1-mini-q4_k_m.gguf`, 462 MiB  
Runtime: local Homebrew `llama-server`, Metal `-ngl 99`, greedy `--temp 0`  
Prompt: exact S1-mini system prompt, empty think block, and
`[Styling: formal] [Structure: lists] [Context: general]`

## Decision

Ship one model and one serial reusable context. Two active Metal slots did not
improve aggregate throughput by the required 20% and approximately doubled
individual request latency. The performance win must come from starting
bounded cleanup during recording, not from concurrent Metal generation.

Production constants:

| Setting                       |                                                              Value |
| ----------------------------- | -----------------------------------------------------------------: |
| Model allocations             |                                                                  1 |
| Active contexts/sequences     |                                                                  1 |
| Queue capacity                |                                                                 16 |
| Queue deadline                |                                                                2 s |
| Per-chunk generation deadline |                                                               12 s |
| Stop finalization deadline    |                                                                5 s |
| Retained revision tail        | 60 words (conservative until tokenizer-tail telemetry replaces it) |

## Concurrency comparison

The same five-times-repeated launch transcript produced 111 output tokens for
every request and byte-identical greedy output.

|         Server slots | Parallel requests | Wall time | Per-request generation | Aggregate generation | Result                                    |
| -------------------: | ----------------: | --------: | ---------------------: | -------------------: | ----------------------------------------- |
|                    1 |                 1 |    1.47 s |             82.0 tok/s |           75.4 tok/s | baseline                                  |
|                    1 |          2 queued |    3.18 s |      83.2 / 63.8 tok/s |           69.9 tok/s | reject                                    |
|                    1 |          4 queued |    6.09 s |        68.5-83.7 tok/s |           73.0 tok/s | reject                                    |
| 2 continuous-batched |                 1 |    1.09 s |            103.4 tok/s |          101.8 tok/s | warm single request                       |
| 2 continuous-batched |                 2 |    2.23 s |        55.8 tok/s each |           99.6 tok/s | reject: no throughput gain, 2x latency    |
| 2 continuous-batched |                 4 |    4.08 s |        52.9-57.2 tok/s |          108.8 tok/s | reject: only 6.9% gain, 3.8x tail latency |

The two-slot result fails the required `>=20%` aggregate-throughput improvement
and makes request latency materially worse. Production therefore remains a
single Metal context with bounded incremental work.

## Warm serial distribution

Ten identical warm requests on one 2,048-token Metal context, prompt cache
disabled:

| Metric                |          Result |
| --------------------- | --------------: |
| Wall p50              |         2.020 s |
| Wall p95 / p99        |         3.851 s |
| Prompt evaluation p50 |      58.7 tok/s |
| Generation p50        |      55.6 tok/s |
| Generation minimum    |      29.0 tok/s |
| Deterministic output  | 10/10 identical |

The variance is why the production per-chunk deadline is 12 seconds and the
stop finalizer is bounded independently at 5 seconds. A missed final deadline
falls back only the unresolved source span and cancels generation at the next
token.

## Quality sanity check

The user-reported uppercase sample retained `TODAY`, `ROUND`, and `GET` when
those uppercase tokens were already present in model input. The same content
sent as raw lowercase ASR was normalized in 1.39 seconds at 92.5 generated
tokens/second with normal sentence casing, punctuation, correction from 10 a.m.
to 9 a.m., and no random uppercase tokens. This confirms the committed T2
ordering fix is essential: S1 must receive raw ASR, not globally rewritten
catalog output.

## Reproduction

Start the server:

```bash
llama-server \
  -m tmp-s1bench/s1-mini-q4_k_m.gguf \
  --host 127.0.0.1 --port 8913 \
  -ngl 99 --ctx-size 2048 --cache-ram 0 \
  --jinja --chat-template-kwargs '{"enable_thinking":false}' \
  --temp 0 -np 1 -cb
```

For the concurrency candidate, change `-np 1` to `-np 2` and issue two or four
identical `/completion` requests concurrently. Keep the raw prompt prefix,
fixture, `n_predict`, and temperature identical.

## Evidence boundary

This benchmark selected the scheduler configuration and exercised the real
GGUF on the target M1. It did not run Parakeet concurrently and is not the T8
30-minute application soak. ASR coexistence, memory pressure, swap, long-session
backlog, and stop-to-paste release gates remain mandatory before a release
`go` decision.

## Accelerated 30-minute replay

A 3,900-word synthetic trace was partitioned into the production scheduling
shape: 22 background spans with a 60-word unresolved tail. Each span contained
a path, explicit number, percentage, and negation invariant.

| Metric                                |     Result |
| ------------------------------------- | ---------: |
| Background spans                      |         22 |
| Accelerated background wall time      |  148.182 s |
| Span wall p50                         |    6.986 s |
| Slowest span                          |    8.826 s |
| Generation p50                        | 73.3 tok/s |
| Stop tail                             |   60 words |
| Stop-tail wall time                   |    1.122 s |
| Stop-tail generation                  | 67.8 tok/s |
| Protected number/path/negation checks |       pass |

This supports the 12-second per-span deadline and shows why incremental work is
the primary latency fix: the same 148 seconds of autoregressive work can happen
during a 30-minute recording, leaving 1.122 seconds at stop in this replay. It
is one accelerated synthetic sample, not a p95 live-ASR claim.
