use std::{env, path::Path, time::Instant};

use transcribe_cpp::{
    Model, ParakeetBufferedStreamOptions, RunOptions, StreamExtension, StreamOptions,
};

const SAMPLE_RATE: usize = 16_000;
// Sweep host feed granularity: 10ms is throughput-optimal per 2026-08-30
// measurement on M1 Metal (30s slice, default geometry), but 30ms is the
// live audio path's natural frame (FrameResampler 30ms). Range covers the
// App's real 30ms and the micro-batch extremes.
const DEFAULT_CHUNK_MS: &[usize] = &[10, 15, 20, 30, 60, 120, 240, 480, 960];

fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|error| error.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE as u32 || spec.bits_per_sample != 16 {
        return Err(format!(
            "expected mono 16 kHz PCM16 WAV, got {} channels at {} Hz / {} bits",
            spec.channels, spec.sample_rate, spec.bits_per_sample
        ));
    }
    reader
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|value| value as f32 / i16::MAX as f32)
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn benchmark(
    model: &Model,
    audio: &[f32],
    chunk_ms: usize,
) -> Result<(f64, f64, String, usize), String> {
    let mut session = model.session().map_err(|error| error.to_string())?;
    let mut stream = session
        .stream(&RunOptions::default(), &StreamOptions::default())
        .map_err(|error| error.to_string())?;
    let chunk_samples = chunk_ms * SAMPLE_RATE / 1_000;
    let started = Instant::now();
    let mut feed_compute_ms = 0.0;
    for chunk in audio.chunks(chunk_samples) {
        let feed_started = Instant::now();
        stream.feed(chunk).map_err(|error| error.to_string())?;
        feed_compute_ms += feed_started.elapsed().as_secs_f64() * 1_000.0;
    }
    stream.finalize().map_err(|error| error.to_string())?;
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    Ok((
        elapsed_ms,
        feed_compute_ms,
        stream.text().full,
        audio.len().div_ceil(chunk_samples),
    ))
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let model_path = args
        .next()
        .ok_or("usage: parakeet_stream_benchmark MODEL.gguf AUDIO.wav")?;
    let audio_path = args
        .next()
        .ok_or("usage: parakeet_stream_benchmark MODEL.gguf AUDIO.wav")?;

    transcribe_cpp::init_logging();
    transcribe_cpp::init_backends_default().map_err(|error| error.to_string())?;
    let audio = load_wav(Path::new(&audio_path))?;
    let audio_seconds = audio.len() as f64 / SAMPLE_RATE as f64;

    let load_started = Instant::now();
    let model = Model::load(Path::new(&model_path)).map_err(|error| error.to_string())?;
    eprintln!(
        "model={} backend={} device={:?} load={:.3}s audio={:.3}s",
        model.arch(),
        model.backend(),
        model.device(),
        load_started.elapsed().as_secs_f64(),
        audio_seconds
    );
    if !model.capabilities().supports_streaming {
        return Err("model does not support streaming".to_string());
    }

    if env::var_os("PARAKEET_PROBE_MENU").is_some() {
        let mut session = model.session().map_err(|error| error.to_string())?;
        let options = StreamOptions {
            family: Some(StreamExtension::ParakeetBuffered(
                ParakeetBufferedStreamOptions {
                    left_ms: Some(0),
                    chunk_ms: Some(0),
                    right_ms: Some(0),
                },
            )),
            ..Default::default()
        };
        return match session.stream(&RunOptions::default(), &options) {
            Ok(_) => Err("unexpectedly accepted zeroed Parakeet geometry".to_string()),
            Err(error) => Err(format!("geometry probe completed: {error}")),
        };
    }

    let warmup_samples = audio.len().min(2 * SAMPLE_RATE);
    let _ = benchmark(&model, &audio[..warmup_samples], 240)?;

    let mut baseline = None;
    println!("chunk_ms,feed_calls,total_ms,feed_compute_ms,rtf,text_similarity,chars");
    for &chunk_ms in DEFAULT_CHUNK_MS {
        let (total_ms, feed_compute_ms, text, feed_calls) = benchmark(&model, &audio, chunk_ms)?;
        let reference = baseline.get_or_insert_with(|| text.clone());
        let similarity = strsim::normalized_levenshtein(reference, &text);
        println!(
            "{chunk_ms},{feed_calls},{total_ms:.3},{feed_compute_ms:.3},{:.6},{similarity:.6},{}",
            total_ms / (audio_seconds * 1_000.0),
            text.chars().count()
        );
    }
    Ok(())
}
