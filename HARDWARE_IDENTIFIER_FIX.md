# Hardware Identifier Protection - Critical Number Normalization Fix

## The Problem

GPU model numbers like `RTX 5000`, `RTX 4090`, `H100` were being corrupted by number normalization:
- `RTX 5000` → `RTX 5,000` ❌
- `RTX 4090` → `RTX 4,090` ❌
- `RTX PRO 6000` → `RTX PRO 6,000` ❌

These are **product identifiers**, not quantities. They must NEVER receive comma separators.

Meanwhile, actual quantities need commas:
- `5000 tokens/s` → `5,000 tokens/s` ✓
- `4096 tok/s` → `4,096 tok/s` ✓

## The Solution

**Architecture: Protect → Normalize → Restore**

1. **Protect** all hardware identifiers BEFORE any number formatting
2. Run quantity normalization only on unprotected spans
3. **Restore** protected identifiers unchanged

### Implementation

```rust
fn normalize_technical_values(text: &str) -> String {
    // Step 1: Protect identifiers with unique placeholders
    let mut out = protect_hardware_identifiers(text);
    
    // Step 2: Run all number normalization
    out = normalize_parameter_counts(&out);
    out = normalize_benchmark_ranges(&out);
    out = normalize_transfer_rates(&out);
    
    // Step 3: Restore protected identifiers
    out = restore_hardware_identifiers(&out);
    
    // ... rest of normalization
}
```

### Protected Identifiers

**NVIDIA RTX Consumer/Gaming:**
- RTX 5090, RTX 5080, RTX 5070
- RTX 4090, RTX 4080, RTX 4070
- RTX 3090, RTX 3080, RTX 3070, RTX 3060

**NVIDIA RTX Pro/Workstation:**
- RTX PRO 6000, RTX PRO 5000, RTX PRO 4000
- RTX 6000, RTX 5000, RTX 4000
- RTX A6000, RTX A5000

**NVIDIA Data Center:**
- H200, H100
- A100, A40, A30, A10

**AMD:**
- Radeon 8060S, Radeon 7900, Radeon 7800

**Platform Identifiers:**
- GB300, GB200
- ConnectX-8, ConnectX-7

## Tech Lexicon Entries

Added comprehensive aliases for all GPU models to `tech_lexicon.json`:

```json
{
  "canonical": "RTX 5090",
  "aliases": [
    "rtx 5090",
    "rtx5090",
    "rtx 5 0 9 0",
    "geforce rtx 5090",
    "nvidia rtx 5090"
  ]
}
```

This ensures:
1. ASR mishearings are corrected to canonical form
2. Number formatting recognizes the pattern
3. Protection mechanism keeps identifiers unchanged

## Regression Tests

Added comprehensive tests in `tests/transcript_normalization_comprehensive.rs`:

### Must Pass - Identifiers Stay Unchanged
```rust
assert_eq!(
    formatter::normalize_values("RTX 5090 is fast"),
    "RTX 5090 is fast"  // NOT "RTX 5,090"
);

assert_eq!(
    formatter::normalize_values("RTX PRO 6000 inside"),
    "RTX PRO 6000 inside"  // NOT "RTX PRO 6,000"
);
```

### Must Pass - Quantities Get Commas
```rust
assert!(formatter::normalize_values("5000 tokens/s").contains("5,000"));
assert!(formatter::normalize_values("4096 tok/s").contains("4,096"));
```

### Must Pass - Mixed Context
```rust
let input = "RTX 4090 generates 4096 tokens per second";
let output = formatter::normalize_values(input);

assert!(output.contains("RTX 4090"));  // Identifier unchanged
assert!(output.contains("4,096"));  // Quantity formatted
```

## Core Principle

**Classify before formatting. Context determines treatment.**

- Preceded by GPU brand/product prefix? → **Identifier** (no commas)
- Followed by unit/quantity suffix? → **Quantity** (add commas)

The protection mechanism ensures identifiers are **never** treated as quantities, even if the number itself would normally receive formatting.

## Impact

### Before Fix
```
RTX 5000 GPU running at 5000 tokens per second
    ↓
RTX 5,000 GPU running at 5,000 tokens per second
```
Both numbers incorrectly formatted ❌

### After Fix
```
RTX 5000 GPU running at 5000 tokens per second
    ↓
RTX 5000 GPU running at 5,000 tokens per second
```
Identifier protected, quantity formatted ✓

## Files Modified

1. **src-tauri/src/catalog/tech_lexicon.json**
   - Added 15+ GPU model entries with aliases
   - Covers NVIDIA RTX, H-series, A-series
   - Covers AMD Radeon
   - Platform identifiers (GB300, ConnectX-8)

2. **src-tauri/src/audio_toolkit/formatter.rs**
   - Added `protect_hardware_identifiers()` function
   - Added `restore_hardware_identifiers()` function
   - Modified `normalize_technical_values()` to use protection
   - Removed old product-specific regex replacements

3. **src-tauri/tests/transcript_normalization_comprehensive.rs**
   - Added `critical_hardware_identifiers_never_get_comma_separators()`
   - Added `quantities_still_get_comma_separators()`
   - Added `mixed_context_hardware_and_quantities()`
   - Added `regression_rtx_5080_never_becomes_5comma080()`

## Testing

Run regression tests:
```bash
cd src-tauri
cargo test critical_hardware_identifiers
cargo test quantities_still_get
cargo test mixed_context_hardware
```

All tests must pass to ensure:
1. No identifier corruption
2. Quantities still normalized correctly
3. Mixed contexts handled properly

## Why This Matters

GPU model numbers in technical transcripts (reviews, benchmarks, tutorials) must be **byte-accurate**. A corrupted model number:
- Breaks product name recognition
- Confuses readers/viewers
- Undermines transcript credibility
- May break downstream parsing/indexing

**Quantities need formatting for readability. Identifiers need protection for accuracy.**

This fix ensures both requirements are met without conflict.
