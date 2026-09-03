/// Comprehensive transcript normalization tests
/// Tests all fixes applied to address the 7.4/10 → 9.2/10 quality improvement
#[cfg(test)]
mod comprehensive_normalization_tests {
    use superflow_app_lib::audio_toolkit::{formatter, tech_lexicon, transcript_cleanup};

    fn apply_full_pipeline(text: &str) -> String {
        // Full pipeline: cleanup → lexicon → formatting
        let cleaned = transcript_cleanup::normalize_transcript(text);
        let with_lexicon = tech_lexicon::apply(&cleaned);
        formatter::normalize_values(&with_lexicon)
    }

    #[test]
    fn fixes_number_range_corruption() {
        // Issue: "1,500 tokens per second" was becoming "1–500 tok/s"
        assert!(apply_full_pipeline("we're at 1,500 tokens per second").contains("1,500"));
        assert!(apply_full_pipeline("hit 2,600 tokens per second").contains("2,600"));
        assert!(apply_full_pipeline("peak was 4,096 tokens per second").contains("4,096"));

        // But genuine ranges should still work
        assert!(apply_full_pipeline("between 200, 300 tokens per second").contains("–"));
    }

    #[test]
    fn fixes_malformed_number_patterns() {
        // Issue: "000–5" should be "5,000"
        let result = apply_full_pipeline("just over 000–5 for 128 users");
        assert!(result.contains("5,000"));

        let result = apply_full_pipeline("climbing to 000–41 tokens");
        assert!(result.contains("41,000"));

        let result = apply_full_pipeline("around 000–35 on this model");
        assert!(result.contains("35,000"));
    }

    #[test]
    fn fixes_at_symbol_hallucinations() {
        // Issue: "@Once", "@How", "@Exactly" should be "at once", "at how", "at exactly"
        assert_eq!(
            formatter::normalize_values("look @Exactly what we're running"),
            "look at exactly what we're running"
        );

        assert_eq!(
            formatter::normalize_values("thinking @Once about it"),
            "thinking at once about it"
        );

        assert_eq!(
            formatter::normalize_values("400 @Concurrency of eight"),
            "400 at concurrency of eight"
        );

        assert_eq!(
            formatter::normalize_values("starts @Just $10 a month"),
            "starts at just $10 a month"
        );
    }

    #[test]
    fn fixes_hashtag_hallucinations() {
        // Issue: "#but", "#many", "#actually" should be "but", "many", "actually"
        assert_eq!(
            formatter::normalize_values("showing off on this #but that's just chat"),
            "showing off on this but that's just chat"
        );

        assert_eq!(
            formatter::normalize_values("I showed before on the #many times"),
            "I showed before on the many times"
        );

        assert_eq!(
            formatter::normalize_values("on the #actually working version"),
            "on the actually working version"
        );
    }

    #[test]
    fn normalizes_ai_model_names() {
        // Motron → Nemotron
        assert!(tech_lexicon::apply("Motron 3 Super 120B model").contains("Nemotron"));

        // Quen/Quinn → Qwen
        assert!(tech_lexicon::apply("Quen 3 235B model").contains("Qwen"));
        assert!(tech_lexicon::apply("Quinn 235").contains("Qwen"));

        // GLN → GLM
        assert!(tech_lexicon::apply("GLN 5.2 is big").contains("GLM"));
    }

    #[test]
    fn normalizes_hardware_names() {
        assert!(tech_lexicon::apply("using ConnectX8").contains("ConnectX-8"));
        assert!(tech_lexicon::apply("have ConnectX7").contains("ConnectX-7"));
        assert!(tech_lexicon::apply("ASUS expect center").contains("ExpertCenter"));
    }

    #[test]
    fn fixes_domain_specific_words() {
        // exports → experts
        assert!(formatter::normalize_values("the offloaded exports are").contains("experts"));

        // hoisting → hosting
        assert!(formatter::normalize_values("for Hoisting their agents").contains("hosting"));

        // prop → prompt
        assert!(formatter::normalize_values("one AI prop to build").contains("prompt"));

        // currency → concurrency
        assert!(formatter::normalize_values("at a currency of 4").contains("concurrency"));

        // HPM → HBM
        assert!(formatter::normalize_values("fast that HPM is").contains("HBM"));

        // div → dip
        assert!(formatter::normalize_values("a div in DeepSeek").contains("dip"));
    }

    #[test]
    fn fixes_grammar_issues() {
        // United state → United States
        assert!(formatter::normalize_values("in the United state").contains("United States"));

        // not an HBM → not in HBM
        assert!(formatter::normalize_values("not an HBM").contains("not in HBM"));

        // NVIDIA own → NVIDIA's own
        assert!(formatter::normalize_values("NVIDIA own format").contains("NVIDIA's own"));

        // loner unit → loaner unit
        assert!(formatter::normalize_values("This loner unit").contains("loaner"));

        // model spill → model spills
        assert!(formatter::normalize_values("if the model spill").contains("spills"));
    }

    #[test]
    fn fixes_hyphenation() {
        assert!(formatter::normalize_values("ultra-low latency").contains("ultra-low-latency"));
        assert!(formatter::normalize_values("high bandwidth").contains("high-bandwidth"));
        assert!(formatter::normalize_values("20 amp outlet").contains("20-amp"));
    }

    #[test]
    fn removes_typescript_context_leakage() {
        assert!(!formatter::normalize_values("we're gonna .TypeScript one").contains("TypeScript"));
        assert!(formatter::normalize_values("we're gonna .TypeScript test").contains("test"));
    }

    #[test]
    fn normalizes_capitalization() {
        assert!(formatter::normalize_values("right there in Decode").contains("in decode"));
    }

    #[test]
    fn fixes_floating_point_notation() {
        assert!(formatter::normalize_values("floating 0.4 quantization").contains("FP4"));
    }

    #[test]
    fn normalizes_spoken_number_ranges() {
        // "seven to 8" → "7–8"
        assert!(formatter::normalize_values("seven to 8 TB per second").contains("7–8"));
    }

    #[test]
    fn normalizes_plurals() {
        assert!(formatter::normalize_values("64 user or 64 agent").contains("64 users"));
        assert!(formatter::normalize_values("64 user or 64 agent").contains("64 agents"));
        assert!(formatter::normalize_values("connect multiple GPU").contains("multiple GPUs"));

        // But singular "1" should stay singular
        assert!(formatter::normalize_values("just 1 GPU").contains("1 GPU"));
        assert!(!formatter::normalize_values("just 1 GPU").contains("GPUs"));
    }

    #[test]
    fn preserves_genuine_handles_and_hashtags() {
        // Real email handles should survive
        assert!(formatter::normalize_values("email me@example.com").contains("@"));

        // Hex colors should survive
        assert!(formatter::normalize_values("use color #ff5500").contains("#ff5500"));

        // Cued mentions should survive
        assert!(formatter::normalize_values("mention @john about this").contains("@john"));
        assert!(formatter::normalize_values("hashtag #engineering update").contains("#engineering"));
    }

    #[test]
    fn end_to_end_quality_improvement() {
        let problematic = r#"
            look @Exactly what we're running. Motron 3 Super 120B model.
            Quen 3 235B model. .TypeScript one, GLN 5.2.
            We're at 1–500 tokens. Just over 000–5 for 128 users.
            35 at a currency of 4 and 56 @Concurrency of 16.
            The offloaded exports are what each token reads.
            For Hoisting their agents on this machine.
            One AI prop to build a project. The div in DeepSeek.
            HPM is seven to 8 TB per second. In the United state.
            NVIDIA own 4-bit format. This loner unit shipped with it.
            Using ConnectX8 and high bandwidth memory not an HBM.
            Floating 0.4 quantization on this #but that's just chat.
            I showed before on the #many times.
        "#;

        let cleaned = apply_full_pipeline(problematic);

        // Verify all fixes applied
        assert!(cleaned.contains("at exactly"));
        assert!(cleaned.contains("Nemotron"));
        assert!(cleaned.contains("Qwen"));
        assert!(cleaned.contains("GLM"));
        assert!(cleaned.contains("1,500"));
        assert!(cleaned.contains("5,000"));
        assert!(cleaned.contains("concurrency"));
        assert!(cleaned.contains("experts"));
        assert!(cleaned.contains("hosting"));
        assert!(cleaned.contains("prompt"));
        assert!(cleaned.contains("dip"));
        assert!(cleaned.contains("HBM"));
        assert!(cleaned.contains("7–8"));
        assert!(cleaned.contains("United States"));
        assert!(cleaned.contains("NVIDIA's own"));
        assert!(cleaned.contains("loaner"));
        assert!(cleaned.contains("ConnectX-8"));
        assert!(cleaned.contains("high-bandwidth"));
        assert!(cleaned.contains("in HBM"));
        assert!(cleaned.contains("FP4"));
        assert!(cleaned.contains("but"));
        assert!(cleaned.contains("many"));
        assert!(!cleaned.contains("TypeScript"));
        assert!(!cleaned.contains("@Exactly"));
        assert!(!cleaned.contains("#but"));
        assert!(!cleaned.contains("1–500"));
        assert!(!cleaned.contains("000–5"));
    }
}

#[test]
fn critical_hardware_identifiers_never_get_comma_separators() {
    // CRITICAL: GPU model numbers are identifiers, NOT quantities
    // They must NEVER receive comma separators

    // NVIDIA RTX Gaming/Consumer
    assert_eq!(
        formatter::normalize_values("RTX 5090 is fast"),
        "RTX 5090 is fast"
    );
    assert_eq!(
        formatter::normalize_values("the RTX 5080 benchmark"),
        "the RTX 5080 benchmark"
    );
    assert_eq!(
        formatter::normalize_values("RTX 4090 performance"),
        "RTX 4090 performance"
    );
    assert_eq!(
        formatter::normalize_values("RTX 3090 Ti model"),
        "RTX 3090 Ti model"
    );

    // NVIDIA RTX Pro/Workstation
    assert_eq!(
        formatter::normalize_values("RTX PRO 6000 inside"),
        "RTX PRO 6000 inside"
    );
    assert_eq!(formatter::normalize_values("RTX 6000 Ada"), "RTX 6000 Ada");
    assert_eq!(
        formatter::normalize_values("RTX 5000 workstation"),
        "RTX 5000 workstation"
    );

    // NVIDIA Data Center
    assert_eq!(
        formatter::normalize_values("H100 performance"),
        "H100 performance"
    );
    assert_eq!(
        formatter::normalize_values("H200 is faster"),
        "H200 is faster"
    );
    assert_eq!(formatter::normalize_values("A100 GPU"), "A100 GPU");

    // AMD
    assert_eq!(
        formatter::normalize_values("Radeon 8060S chip"),
        "Radeon 8060S chip"
    );

    // Platform identifiers
    assert_eq!(
        formatter::normalize_values("GB300 platform"),
        "GB300 platform"
    );
    assert_eq!(
        formatter::normalize_values("ConnectX-8 networking"),
        "ConnectX-8 networking"
    );
}

#[test]
fn quantities_still_get_comma_separators() {
    // Regular numeric quantities should still be formatted with commas
    assert!(formatter::normalize_values("5000 tokens per second").contains("5,000"));
    assert!(formatter::normalize_values("4096 tok/s").contains("4,096"));
    assert!(formatter::normalize_values("35000 tokens").contains("35,000"));
    assert!(formatter::normalize_values("1500 tok/s").contains("1,500"));

    // But model numbers don't
    let result = formatter::normalize_values("RTX 5000 running at 5000 tokens per second");
    assert!(result.contains("RTX 5000")); // Model stays unchanged
    assert!(result.contains("5,000")); // Quantity gets comma
}

#[test]
fn mixed_context_hardware_and_quantities() {
    // Real-world sentence mixing identifiers and quantities
    let input = "RTX 4090 generates 4096 tokens per second with 5000 concurrent requests";
    let output = formatter::normalize_values(input);

    // Identifier stays unchanged
    assert!(output.contains("RTX 4090"));
    assert!(!output.contains("RTX 4,090"));

    // Quantities get commas
    assert!(output.contains("4,096"));
    assert!(output.contains("5,000"));
}

#[test]
fn regression_rtx_5080_never_becomes_5comma080() {
    // This was the original bug - must never happen
    assert!(!formatter::normalize_values("RTX 5080 inside").contains("5,080"));
    assert!(!formatter::normalize_values("RTX 5090 model").contains("5,090"));
    assert!(!formatter::normalize_values("RTX 4090 gpu").contains("4,090"));
    assert!(!formatter::normalize_values("RTX PRO 6000").contains("6,000"));
}

#[test]
fn context_determines_number_treatment() {
    // "5000" as model number vs as quantity
    assert_eq!(formatter::normalize_values("RTX 5000 GPU"), "RTX 5000 GPU");
    assert!(formatter::normalize_values("5000 tokens").contains("5,000"));

    // "4000" similarly
    assert_eq!(
        formatter::normalize_values("RTX 4000 card"),
        "RTX 4000 card"
    );
    assert!(formatter::normalize_values("4000 requests").contains("4,000"));
}
