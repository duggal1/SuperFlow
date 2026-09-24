use superflow_app_lib::audio_toolkit::{
    emoji, formatter, styling, tech_lexicon, transcript_cleanup,
};
use superflow_app_lib::superflow_grammar::{correct, ProtectedText};

fn main() {
    let raw = r#"Last year with the X2 still holds up on the X3. So I grabbed the model I already tested on the X2 last year. It's older, yeah, but I wanted to make sure.

Qwen2.5, 32B parameters. On the old X2, it ran about 11 tok/s. On the X3, 11.2. Same chip, same speed. And that's a good thing, which makes the rest of the testing reliable.

One quick note for the nerds: all my tests are going to be using Llama C++ and not Ollama, even though I did test Ollama last year.

Ollama is convenient, but on this hardware, it's slower. The numbers check out. Let's get to the fun part. It's too big. Yeah, look at that. We're using all the memory.

We're using 100% of the GPU and nothing is being spit out at all. But it is being spit out. It's just very, very slow.

nvtop is showing that we are using the GPU to the fullest here. Runs both GPUs exactly the same way that it does on Linux.

I installed Linux right away on there in a dual boot scenario, but I did test first."#;

    println!("=== RAW ===");
    println!("{}", raw);
    println!("\n=== PIPELINE ===");

    // Simulate post_process steps
    let protected = ProtectedText::new(raw);
    let masked = protected.masked().to_string();
    let harpered = correct(&masked);
    let restored = protected.restore(&harpered);
    println!("\n--- after harper ---");
    println!("{}", restored);

    let step1 = tech_lexicon::apply(&restored);
    println!("\n--- after tech_lexicon ---");
    println!("{}", step1);

    let step2 = styling::apply(&step1);
    println!("\n--- after styling ---");
    println!("{}", step2);

    let step4 = emoji::apply(&step2);
    println!("\n--- after emoji ---");
    println!("{}", step4);

    // Use default settings for filler removal - assume English
    use superflow_app_lib::audio_toolkit::text::{
        join_path_tokens, normalize_transcription_output, remove_filler_words,
        OutputLanguageEvidence,
    };
    let lang = OutputLanguageEvidence::UserSelected("en".to_string());
    let without_fillers = remove_filler_words(&step4, &lang, &None, true);
    println!("\n--- after remove_fillers ---");
    println!("{}", without_fillers);

    let normalized = normalize_transcription_output(&without_fillers);
    println!("\n--- after normalize_transcription_output ---");
    println!("{}", normalized);

    let normalized2 = formatter::normalize_values(&normalized);
    println!("\n--- after normalize_values ---");
    println!("{}", normalized2);

    let joined = join_path_tokens(&normalized2);
    println!("\n--- after join_path_tokens ---");
    println!("{}", joined);

    let cleaned = transcript_cleanup::normalize_transcript(&joined);
    println!("\n--- after transcript_cleanup ---");
    println!("{}", cleaned);

    // Format layout
    let formatted = formatter::format_layout(&cleaned);
    println!("\n=== FINAL FORMATTED ===");
    println!("{}", formatted);

    // Also test paragraph groups directly
    println!("\n=== PARAGRAPH WORD COUNTS ===");
    for (i, para) in formatted.split("\n\n").enumerate() {
        let wc = para.split_whitespace().count();
        println!("Para {}: {} words", i + 1, wc);
    }
}
