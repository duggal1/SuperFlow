use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Compressed, rotating transcript store.
///
/// Format: `.sp/transcripts/<mission_id>/<worker>.log`
/// Written as plain text (agents produce terminal output, we just store it).
/// Rotation at 500MB per file. Old files can be compressed with zstd manually.
pub struct TranscriptStore {
    base_dir: PathBuf,
    max_file_size: u64, // 500MB default
}

impl TranscriptStore {
    pub fn open(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)
            .with_context(|| format!("failed to create transcript dir {}", base_dir.display()))?;
        Ok(Self {
            base_dir,
            max_file_size: 500 * 1024 * 1024, // 500MB
        })
    }

    pub fn mission_dir(&self, mission_id: &str) -> PathBuf {
        self.base_dir.join(mission_id)
    }

    pub fn transcript_file(&self, mission_id: &str, worker_name: &str, index: u64) -> PathBuf {
        if index == 0 {
            self.mission_dir(mission_id)
                .join(format!("{}.log", worker_name))
        } else {
            self.mission_dir(mission_id)
                .join(format!("{}.log.{}", worker_name, index))
        }
    }

    pub fn get_or_create_writer(
        &self,
        mission_id: &str,
        worker_name: &str,
    ) -> Result<TranscriptWriter<'_>> {
        let dir = self.mission_dir(mission_id);
        std::fs::create_dir_all(&dir)?;

        // Find current file (not rotated)
        let mut index = 0u64;
        loop {
            let file = self.transcript_file(mission_id, worker_name, index);
            let size = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            if size < self.max_file_size {
                let f = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file)?;
                return Ok(TranscriptWriter {
                    file: BufWriter::new(f),
                    current_size: size,
                    max_size: self.max_file_size,
                    mission_id: mission_id.to_owned(),
                    worker_name: worker_name.to_owned(),
                    index,
                    store: self,
                });
            }
            index += 1;
        }
    }

    pub fn list_transcripts(&self, mission_id: &str) -> Result<Vec<String>> {
        let dir = self.mission_dir(mission_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".log") || name.contains(".log.") {
                // Extract worker name
                let worker = name.split(".log").next().unwrap_or(&name);
                if !names.contains(&worker.to_owned()) {
                    names.push(worker.to_owned());
                }
            }
        }
        Ok(names)
    }

    pub fn purge_old(&self, max_age_days: u64) -> Result<usize> {
        if !self.base_dir.exists() {
            return Ok(0);
        }

        let cutoff =
            std::time::SystemTime::now() - std::time::Duration::from_secs(max_age_days * 24 * 3600);
        let mut count = 0;

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                for file_entry in fs::read_dir(entry.path())? {
                    let file_entry = file_entry?;
                    if let Ok(metadata) = file_entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if modified < cutoff {
                                fs::remove_file(file_entry.path())?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }
}

pub struct TranscriptWriter<'a> {
    file: BufWriter<fs::File>,
    current_size: u64,
    max_size: u64,
    mission_id: String,
    worker_name: String,
    index: u64,
    store: &'a TranscriptStore,
}

impl TranscriptWriter<'_> {
    pub fn write(&mut self, text: &str) -> Result<()> {
        self.file.write_all(text.as_bytes())?;
        self.current_size += text.len() as u64;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.file.flush()?;
        Ok(())
    }

    pub fn rotate_if_needed(&mut self) -> Result<()> {
        if self.current_size >= self.max_size {
            self.file.flush()?;
            self.index += 1;
            let new_file =
                self.store
                    .transcript_file(&self.mission_id, &self.worker_name, self.index);
            let f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_file)?;
            self.file = BufWriter::new(f);
            self.current_size = 0;
        }
        Ok(())
    }
}
