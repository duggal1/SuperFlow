use anyhow::{Context, Result};
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Read a WAV file and return normalised f32 samples.
pub fn read_wav_samples<P: AsRef<Path>>(file_path: P) -> Result<Vec<f32>> {
    let reader = WavReader::open(file_path.as_ref())?;
    let samples = reader
        .into_samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<Vec<f32>, _>>()?;
    Ok(samples)
}

/// Open a raw little-endian f32 journal for appending (16 kHz mono). The
/// counterpart of [`read_f32_part`]; used to keep in-flight recordings on
/// disk so a hard crash mid-dictation never loses the audio.
pub fn create_f32_part<P: AsRef<Path>>(file_path: P) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(file_path.as_ref())?))
}

/// Append raw f32 samples to a journal writer. Errors are surfaced; the
/// caller decides whether to disable the journal and keep recording.
pub fn append_f32_part(writer: &mut BufWriter<File>, samples: &[f32]) -> std::io::Result<()> {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    writer.write_all(&bytes)
}

/// Read a raw f32 journal written by [`append_f32_part`]. Tolerates a torn
/// final sample (a crash can land mid-write): trailing bytes that do not
/// form a whole f32 are ignored.
pub fn read_f32_part<P: AsRef<Path>>(file_path: P) -> Result<Vec<f32>> {
    let mut bytes = Vec::new();
    BufReader::new(File::open(file_path.as_ref())?)
        .read_to_end(&mut bytes)
        .with_context(|| format!("Failed to read recording journal {:?}", file_path.as_ref()))?;
    let whole = bytes.len() - (bytes.len() % 4);
    Ok(bytes[..whole]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        // A NaN from a torn write would poison downstream processing.
        .filter(|s| s.is_finite())
        .collect())
}

/// Verify a WAV file by reading it back and checking the sample count.
pub fn verify_wav_file<P: AsRef<Path>>(file_path: P, expected_samples: usize) -> Result<()> {
    let reader = WavReader::open(file_path.as_ref())?;
    let actual_samples = reader.len() as usize;
    if actual_samples != expected_samples {
        anyhow::bail!(
            "WAV sample count mismatch: expected {}, got {}",
            expected_samples,
            actual_samples
        );
    }
    Ok(())
}

/// Save audio samples as a WAV file
pub fn save_wav_file<P: AsRef<Path>>(file_path: P, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(file_path.as_ref(), spec)?;

    // Convert f32 samples to i16 for WAV
    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;
    debug!("Saved WAV file: {:?}", file_path.as_ref());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_part_round_trip_survives_torn_tail() {
        let dir = std::env::temp_dir().join("superflow_f32part_test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("roundtrip.f32part");

        let samples: Vec<f32> = (0..1000).map(|i| (i as f32) / 500.0 - 1.0).collect();
        {
            let mut writer = create_f32_part(&path).expect("create journal");
            append_f32_part(&mut writer, &samples[..600]).expect("append first chunk");
            append_f32_part(&mut writer, &samples[600..]).expect("append second chunk");
        }

        let read_back = read_f32_part(&path).expect("read journal");
        assert_eq!(read_back.len(), samples.len());
        for (a, b) in read_back.iter().zip(samples.iter()) {
            assert_eq!(a.to_le_bytes(), b.to_le_bytes());
        }

        // Simulate a torn final write: truncate to a non-multiple of 4.
        let mut bytes = std::fs::read(&path).expect("read raw");
        bytes.truncate(bytes.len() - 2);
        std::fs::write(&path, &bytes).expect("write torn file");
        let torn = read_f32_part(&path).expect("torn read still succeeds");
        assert_eq!(torn.len(), samples.len() - 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
