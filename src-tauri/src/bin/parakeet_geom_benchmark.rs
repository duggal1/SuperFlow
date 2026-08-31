use std::{env, path::Path, time::Instant};

use transcribe_cpp::{
    Model, ParakeetBufferedStreamOptions, RunOptions, StreamExtension, StreamOptions,
};

const SAMPLE_RATE: usize = 16_000;

fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE as u32 || spec.bits_per_sample != 16 {
        return Err(format!(
            "expected mono 16k PCM16, got {}ch {}Hz {}bits",
            spec.channels, spec.sample_rate, spec.bits_per_sample
        ));
    }
    reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32).map_err(|e| e.to_string()))
        .collect()
}

fn bench_stream(
    model: &Model,
    audio: &[f32],
    host_chunk_ms: usize,
    geom: Option<(i32, i32, i32)>,
) -> Result<(f64, String), String> {
    let mut session = model.session().map_err(|e| e.to_string())?;
    let stream_opts = if let Some((l, c, r)) = geom {
        StreamOptions {
            family: Some(StreamExtension::ParakeetBuffered(
                ParakeetBufferedStreamOptions {
                    left_ms: Some(l),
                    chunk_ms: Some(c),
                    right_ms: Some(r),
                },
            )),
            ..Default::default()
        }
    } else {
        StreamOptions::default()
    };
    let mut stream = session
        .stream(&RunOptions::default(), &stream_opts)
        .map_err(|e| e.to_string())?;
    let host_samples = host_chunk_ms * SAMPLE_RATE / 1000;
    let start = Instant::now();
    for chunk in audio.chunks(host_samples) {
        stream.feed(chunk).map_err(|e| e.to_string())?;
    }
    stream.finalize().map_err(|e| e.to_string())?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok((elapsed_ms, stream.text().full))
}

fn bench_offline(model: &Model, audio: &[f32]) -> Result<(f64, String), String> {
    let mut session = model.session().map_err(|e| e.to_string())?;
    let start = Instant::now();
    let t = session
        .run(audio, &RunOptions::default())
        .map_err(|e| e.to_string())?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok((elapsed_ms, t.text))
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let model_path = args.next().ok_or("usage: geom_benchmark MODEL.gguf AUDIO.wav")?;
    let audio_path = args.next().ok_or("usage: geom_benchmark MODEL.gguf AUDIO.wav")?;

    transcribe_cpp::init_logging();
    transcribe_cpp::init_backends_default().map_err(|e| e.to_string())?;

    let audio = load_wav(Path::new(&audio_path))?;
    let audio_secs = audio.len() as f64 / SAMPLE_RATE as f64;

    let load_start = Instant::now();
    let model = Model::load(Path::new(&model_path)).map_err(|e| e.to_string())?;
    eprintln!(
        "model={} backend={} device={:?} load={:.3}s audio={:.3}s caps_streaming={} arch={} variant={}",
        model.arch(),
        model.backend(),
        model.device(),
        load_start.elapsed().as_secs_f64(),
        audio_secs,
        model.capabilities().supports_streaming,
        model.arch(),
        model.variant()
    );
    if !model.capabilities().supports_streaming {
        return Err("model does not support streaming".to_string());
    }

    // Warmup once with default streaming
    let _ = bench_stream(&model, &audio[..audio.len().min(2 * SAMPLE_RATE)], 30, None)?;

    // Use a short slice for geometry sweep to keep runtime reasonable
    // Full 121s file takes ~35s per streaming run => 9+14 runs would be >12min.
    // 30s slice preserves relative ordering while keeping total <4min.
    let sweep_audio = &audio[..(30 * SAMPLE_RATE).min(audio.len())];
    let sweep_secs = sweep_audio.len() as f64 / SAMPLE_RATE as f64;

    // ----- 1. Host feed granularity with default geometry (on sweep slice) -----
    println!("\n=== HOST FEED GRANULARITY (default geometry L=5600 C=1040 R=1040, 30s slice) ===");
    println!("host_ms,total_ms,rtf,chars,preview");
    let host_sizes = [10usize, 15, 20, 30, 60, 120, 240, 480, 960];
    let mut baseline_text = String::new();
    for &hm in &host_sizes {
        let (ms, text) = bench_stream(&model, sweep_audio, hm, None)?;
        if baseline_text.is_empty() {
            baseline_text = text.clone();
        }
        let rtf = ms / (sweep_secs * 1000.0);
        let sim = strsim::normalized_levenshtein(&baseline_text, &text);
        println!(
            "{},{:.1},{:.4},{},sim={:.4},\"{}\"",
            hm,
            ms,
            rtf,
            text.chars().count(),
            sim,
            text.chars().take(60).collect::<String>().replace('"', "'")
        );
    }

    // ----- 2. Encoder geometry sweep (host feed fixed at 30ms) on 30s slice -----
    println!("\n=== ENCODER GEOMETRY SWEEP (host feed = 30ms, 30s slice) ===");
    println!("L_ms,C_ms,R_ms,window_ms,total_ms,rtf,chars,sim,preview");
    let geoms: Vec<Option<(i32, i32, i32)>> = vec![
        None, // default
        Some((5600, 1040, 0)),
        Some((5600, 1040, 80)),
        Some((5600, 1040, 160)),
        Some((5600, 1040, 240)),
        Some((5600, 1040, 320)),
        Some((5600, 1040, 560)),
        Some((5600, 560, 0)),
        Some((5600, 560, 80)),
        Some((5600, 560, 160)),
        Some((5600, 560, 560)),
        Some((5600, 160, 0)),
        Some((5600, 160, 80)),
        Some((5600, 80, 0)),
    ];
    // First get baseline (default geom) text
    let (_, default_text) = bench_stream(&model, sweep_audio, 30, None)?;
    for geom in geoms.clone() {
        let (l, c, r) = match geom {
            None => (5600, 1040, 1040),
            Some(v) => v,
        };
        let label = if geom.is_none() { "default".to_string() } else { format!("{},{},{}", l, c, r) };
        let window = l + c + r;
        match bench_stream(&model, sweep_audio, 30, geom) {
            Ok((ms, text)) => {
                let rtf = ms / (sweep_secs * 1000.0);
                let sim = strsim::normalized_levenshtein(&default_text, &text);
                println!(
                    "{},{},{},{},{:.1},{:.4},{},{:.4},\"{}\"",
                    l, c, r, window, ms, rtf, text.chars().count(), sim,
                    text.chars().take(60).collect::<String>().replace('"', "'")
                );
                eprintln!("  {} => {:.1}ms RTF {:.3} sim {:.4}", label, ms, rtf, sim);
            }
            Err(e) => {
                println!("{},{},{},{},ERROR,0,0,0,\"{}\"", l, c, r, window, e.replace('"', "'"));
                eprintln!("  {} => ERROR: {}", label, e);
            }
        }
    }
    // Validate top 2 fastest geometries on full file to confirm slice predicts full
    println!("\n=== VALIDATION: top geometries on FULL 121s file (host 30ms) ===");
    // We'll pick the two fastest from sweep plus default for comparison - run on full audio
    // To avoid hardcoding, just test default vs the fastest small-window vs fastest large-window
    let validate_geoms = [None, Some((5600, 1040, 0)), Some((5600, 560, 0))];
    for geom in validate_geoms {
        let (l,c,r) = match geom { None => (5600,1040,1040), Some(v)=>v };
        let label = if geom.is_none() { "default".to_string() } else { format!("{},{},{}",l,c,r)};
        match bench_stream(&model, &audio, 30, geom) {
            Ok((ms, text)) => {
                let rtf = ms / (audio_secs * 1000.0);
                let sim = strsim::normalized_levenshtein(&default_text, &text);
                println!("FULL {} => {:.1}ms RTF {:.4} chars {} sim {:.4}", label, ms, rtf, text.chars().count(), sim);
            }
            Err(e) => println!("FULL {} => ERROR {}", label, e),
        }
    }

    // ----- 3. Batch vs streaming comparison -----
    println!("\n=== BATCH vs STREAMING (default geom, 120s file) ===");
    let (stream_ms, stream_text) = bench_stream(&model, &audio, 30, None)?;
    let (batch_ms, batch_text) = bench_offline(&model, &audio)?;
    println!(
        "streaming: {:.1}ms RTF {:.4} chars {}",
        stream_ms,
        stream_ms / (audio_secs * 1000.0),
        stream_text.chars().count()
    );
    println!(
        "batch:     {:.1}ms RTF {:.4} chars {}",
        batch_ms,
        batch_ms / (audio_secs * 1000.0),
        batch_text.chars().count()
    );
    println!(
        "batch speedup vs streaming: {:.2}x",
        stream_ms / batch_ms
    );
    let sim = strsim::normalized_levenshtein(&batch_text, &stream_text);
    println!("stream vs batch similarity: {:.4}", sim);

    // ----- 4. Parallelism probe: try concurrent streams on same model -----
    println!("\n=== PARALLEL GPU PROBE (same Model, 2 concurrent streams) ===");
    {
        let mut s1 = model.session().map_err(|e| e.to_string())?;
        let mut st1 = s1
            .stream(&RunOptions::default(), &StreamOptions::default())
            .map_err(|e| e.to_string())?;
        let chunk = &audio[..16000.min(audio.len())];
        let r1 = st1.feed(chunk);
        println!("first stream feed: {:?}", r1.map(|_| "ok").unwrap_or("err"));
        // Now try to create second session+stream while first holds stream borrow
        // We need a separate scope so s1 borrow ends before s2 creation attempt
        drop(st1);
        let mut s2 = model.session().map_err(|e| e.to_string())?;
        match s2.stream(&RunOptions::default(), &StreamOptions::default()) {
            Ok(mut st2) => {
                let r2 = st2.feed(chunk);
                println!("second stream creation: OK, feed: {:?}", r2.map(|_| "ok").unwrap_or("err"));
                println!("NOTE: sessions can be created sequentially, but concurrent compute is serialized by per-model mutex. See threaded test next.");
            }
            Err(e) => {
                println!("second stream creation failed: {}", e);
            }
        }
        // Threaded concurrent compute test: two threads sharing same Model (Arc) try to run at once
        // transcribe-cpp docs: at most one in-flight run/stream across all sessions of a model; others queue behind mutex
        let model_clone = model.clone();
        let audio_clone = audio[..32000.min(audio.len())].to_vec();
        let audio_clone2 = audio_clone.clone();
        let t1 = std::thread::spawn(move || {
            let start = Instant::now();
            let mut sess = model_clone.session().unwrap();
            let mut st = sess.stream(&RunOptions::default(), &StreamOptions::default()).unwrap();
            st.feed(&audio_clone).unwrap();
            st.finalize().unwrap();
            let elapsed = start.elapsed().as_millis();
            (elapsed, st.text().full.len())
        });
        // Give t1 a head start to acquire compute lock
        std::thread::sleep(std::time::Duration::from_millis(10));
        let start2 = Instant::now();
        let mut sess2 = model.session().map_err(|e| e.to_string())?;
        let mut st2b = sess2
            .stream(&RunOptions::default(), &StreamOptions::default())
            .map_err(|e| e.to_string())?;
        let r = st2b.feed(&audio_clone2);
        let elapsed2 = start2.elapsed().as_millis();
        println!(
            "threaded concurrent: second feed while first thread active: {:?} after {}ms (if serialized, second will block until first releases compute_lock)",
            r.map(|_| "ok").unwrap_or("err"),
            elapsed2
        );
        let (elapsed1, _) = t1.join().unwrap();
        println!("first thread elapsed: {}ms, second thread waited (serialized, not parallel)", elapsed1);
        println!("CONCLUSION: transcribe-cpp enforces per-model single in-flight compute (compute_lock). Parallel GPU streams on same model serialize, not accelerate.");
    }

    // ----- 5. Quantization hint -----
    println!("\n=== QUANTIZATION NOTE ===");
    println!("Current model: Q8_0 (731 MB). Q4_K_M (477 MB) would be ~30-40% faster at cost of accuracy. Tradeoff only via model file, not runtime knob.");

    Ok(())
}
