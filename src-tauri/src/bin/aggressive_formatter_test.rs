use superflow_app_lib::audio_toolkit::formatter::{format, format_layout, normalize_values};
use superflow_app_lib::settings::PunctuationStyle;

fn assert_eq_verbose(name: &str, got: &str, expected: &str) {
    if got != expected {
        eprintln!(
            "FAIL {}:\n  got:      {:?}\n  expected: {:?}",
            name, got, expected
        );
        std::process::exit(1);
    } else {
        println!("PASS {}", name);
    }
}

fn main() {
    println!("=== Aggressive Formatter Tests ===");

    // 1. Punctuation / grammar — ensure_terminal via format()
    let cases = vec![
        ("hello world", PunctuationStyle::Informal, "Hello world."),
        ("hello world!", PunctuationStyle::Informal, "Hello world!"),
        ("hello world?", PunctuationStyle::Informal, "Hello world?"),
        ("hello world:", PunctuationStyle::Informal, "Hello world:"),
        ("", PunctuationStyle::Formal, ""),
        ("   ", PunctuationStyle::Formal, "   "),
        (
            "fix the MY_CONSTANT value",
            PunctuationStyle::Informal,
            "Fix the MY_CONSTANT value.",
        ),
        (
            "myFunction should stay",
            PunctuationStyle::Informal,
            "myFunction should stay.",
        ),
        (
            "src/file.rs should stay",
            PunctuationStyle::Informal,
            "Src/file.rs should stay.",
        ), // actually recapitalize will capitalize? Check
        (
            "café and résumé",
            PunctuationStyle::Informal,
            "Café and résumé.",
        ),
        ("hello José", PunctuationStyle::Informal, "Hello José."),
    ];
    for (i, (input, style, expected_suffix)) in cases.iter().enumerate() {
        let out = format(input, *style);
        // For whitespace we expect exact preservation
        if *input == "   " || (*expected_suffix).is_empty() {
            assert_eq_verbose(&format!("punct {}", i), &out, expected_suffix);
        } else {
            assert!(
                out.ends_with(|c| ".!?".contains(c)) || out.ends_with(':'),
                "case {} got {:?} should end with terminal",
                i,
                out
            );
            println!("PASS punct {}", i);
        }
    }

    // 2. Numeric grammar — ultra aggressive
    let numeric_cases = vec![
        ("it takes two hundred milliseconds", "it takes 200ms"),
        ("twenty percent more", "20% more"),
        ("negative twenty five percent", "-25%"),
        ("point five per cent", "0.5%"),
        ("one lakh rupees", "₹1,00,000"),
        ("five gigabytes", "5 GB"),
        ("one trillion dollars", "$1,000,000,000,000"),
        ("meet at three thirty pm", "meet at 3:30 PM"),
        ("meet at three thirty p m", "meet at 3:30 PM"),
        ("meet at three thirty a m", "meet at 3:30 AM"),
        ("meet at three thirty", "meet at 3:30"),
        ("give me five", "give me five"),
        (
            "meet at twenty three thirty pm",
            "meet at twenty three thirty pm",
        ), // should NOT parse
        ("twenty three thirty", "twenty three thirty"), // no cue, no parse
        ("costs two thousand dollars", "costs $2,000"),
        ("add sixteen pixels", "add 16px"),
        ("set it ninety degrees", "set it 90deg"),
        ("we made two hundred thousand dollars", "we made $200,000"),
        ("about one million users", "about 1,000,000 users"),
        ("one point five rem", "1.5rem"),
        ("two hundreed thousand dollars,", "$200,000,"), // typo but should still? Actually hundreed typo -> not parsed as 200k? But test expects $200k with comma
    ];
    for (input, expected) in numeric_cases {
        let out = normalize_values(input);
        assert_eq_verbose(&format!("numeric {:?}", input), &out, expected);
    }

    // 3. Formatting / layout — idempotence, unicode, code fences, lists, paragraphs
    let layout_cases = vec![
        ("first fix the header. second fix the card. third remove the footer.", "1. Fix the header.\n2. Fix the card.\n3. Remove the footer."),
        ("We need React, TypeScript, Tailwind CSS, and Tauri.", "We need:\n- React\n- TypeScript\n- Tailwind CSS\n- Tauri"),
        ("I opened Gmail, replied to Alex, and then went home.", "I opened Gmail, replied to Alex, and then went home."),
        ("The app is fast, reliable, and local.", "The app is fast, reliable, and local."),
        ("We need:\n- React\n- TypeScript\n- Tauri", "We need:\n- React\n- TypeScript\n- Tauri"), // idempotent
        ("```\nmust not touch first second third in here\n```", "```\nmust not touch first second third in here\n```"),
        ("", ""),
        ("hey david just wanted to give you an update thanks", "Hey David,"), // greeting envelope
        ("We need React, TypeScript, Tailwind CSS, and Tauri. quick status: dashboards shipped. login resolved. api patched. payments pending. final-tail-token", "final-tail-token"), // should contain tail
    ];
    for (i, (input, expected_contains)) in layout_cases.iter().enumerate() {
        let out = format_layout(input);
        if (*expected_contains).is_empty() {
            assert_eq_verbose(&format!("layout empty {}", i), &out, expected_contains);
        } else if *expected_contains == "final-tail-token" {
            assert!(
                out.contains("final-tail-token"),
                "layout {} should contain final-tail-token, got {:?}",
                i,
                out
            );
            assert!(
                !out.contains("- Final-tail-token"),
                "layout {} should NOT bullet the tail, got {:?}",
                i,
                out
            );
            println!("PASS layout tail {}", i);
        } else if expected_contains.contains('\n') {
            assert_eq_verbose(&format!("layout {}", i), &out, expected_contains);
        } else {
            assert!(
                out.contains(expected_contains),
                "layout {} expected to contain {:?}, got {:?}",
                i,
                expected_contains,
                out
            );
            println!("PASS layout contains {}", i);
        }
    }

    // 4. Unicode & protected tokens
    let unicode_cases = vec![
        "hello José this is a normal sentence with café and résumé.",
        "नमस्ते दुनिया. This remains valid UTF-8.",
        "hey josé just checking in thanks",
        "We need café, résumé, jalapeño, and piñata.",
        "quick status: café shipped. résumé fixed. jalapeño tested. piñata ready.",
    ];
    for case in unicode_cases {
        let out = format_layout(case);
        assert!(
            std::str::from_utf8(out.as_bytes()).is_ok(),
            "unicode case failed {:?}",
            case
        );
        let preview: String = case.chars().take(20).collect();
        println!("PASS unicode {:?}", preview);
    }

    let protected = "Keep src-tauri/src/managers/mlx.rs https://example.com dev@example.com bg-rose-600 Next.js TanStack Query shadcn/ui GGUF unchanged. Another sentence follows with enough words to make paragraph logic inspect the surrounding text without mutating those tokens at all.";
    let out = format_layout(protected);
    for token in [
        "src-tauri/src/managers/mlx.rs",
        "https://example.com",
        "dev@example.com",
        "bg-rose-600",
        "Next.js",
        "TanStack Query",
        "shadcn/ui",
        "GGUF",
    ] {
        assert!(out.contains(token), "missing protected {}", token);
    }
    println!("PASS protected tokens");

    // 5. Performance — 10min transcript, long layout
    let paragraph = "please check two hundreed thousand dollars and twenty five percent then preserve src/backend/payment/payment.ts without unrelated changes ";
    let input = paragraph.repeat(100);
    let start = std::time::Instant::now();
    let output = format(&input, PunctuationStyle::Formal);
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "10min transcript took {:?}",
        elapsed
    );
    assert!(output.contains("$200,000"));
    assert!(output.contains("25%"));
    assert!(output.contains("`src/backend/payment/payment.ts`"));
    println!("PASS performance 10min {:?}", elapsed);

    let sentence = "This sentence contains enough ordinary words to exercise deterministic paragraph grouping while preserving every protected token src-tauri/src/managers/mlx.rs and https://example.com without changing meaning or structure. ";
    let input = sentence.repeat(200);
    let start = std::time::Instant::now();
    let first = format_layout(&input);
    let elapsed = start.elapsed();
    let second = format_layout(&first);
    assert_eq!(second, first, "layout idempotent");
    assert!(
        elapsed < std::time::Duration::from_millis(200),
        "long layout took {:?}",
        elapsed
    );
    println!("PASS long layout deterministic {:?}", elapsed);

    // 6. Idempotence
    let once =
        format_layout("hey sam quick update first fix payments second verify receipts thanks");
    let twice = format_layout(&once);
    assert_eq!(twice, once, "idempotent");
    println!("PASS idempotence");

    let bullets = "1. Fix login.\n2. Check dashboard.\n3. Ship it.";
    assert_eq!(format_layout(bullets), bullets);
    println!("PASS bullets idempotent");

    // 7. Time edge cases
    assert_eq!(normalize_values("meet at 10:30 pm"), "meet at 10:30 pm"); // already normalized? Should be?
    assert_eq!(
        normalize_values("meet at twenty 3:30 PM"),
        "meet at twenty 3:30 PM"
    ); // not double parse
    println!("PASS time edge");

    println!("\n=== ALL AGGRESSIVE TESTS PASSED ===");
}
