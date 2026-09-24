use superflow_app_lib::audio_toolkit::{styling, tech_lexicon};

fn main() {
    let tests = vec![
        "Qwen2.5",
        "Qwen2.5,",
        "Qwen2.5, 32B parameters",
        "Llama C++",
        "Llama C++ and not Ollama",
        "RTX 5080",
        "RTX PRO 6000",
        "Oculink",
        "GMK Tech",
        "Evil X2",
        "Evil X3",
        "Nvidia",
        "P C I E",
        "V RAM",
    ];
    for t in tests {
        let out = tech_lexicon::apply(t);
        println!("tech_lexicon: {:?} -> {:?}", t, out);
    }
    println!("--- styling ---");
    for t in vec!["bg-stone-600", "white"] {
        println!("styling: {:?} -> {:?}", t, styling::apply(t));
    }
}
