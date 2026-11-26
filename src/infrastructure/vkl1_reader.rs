use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use crate::domain::error::DomainError;
use crate::domain::ports::CandleSeriesReader;
use crate::domain::types::Candle;
use crate::infrastructure::binary_codec::{decode_vkl1_record, VKL1_RECORD_BYTES};

pub struct StreamingVkl1Reader {
    files: Vec<PathBuf>,
    file_idx: usize,
    reader: Option<BufReader<File>>,
}

impl StreamingVkl1Reader {
    pub fn open(path: &Path) -> Result<Self, DomainError> {
        let meta = fs::metadata(path)
            .map_err(|e| DomainError::Io(format!("stat {}: {e}", path.display())))?;
        let files = if meta.is_dir() {
            collect_and_sort_vkl1_files(path)?
        } else {
            validate_vkl1_len(meta.len(), path)?;
            vec![path.to_path_buf()]
        };
        Ok(Self {
            files,
            file_idx: 0,
            reader: None,
        })
    }

    fn open_next_file(&mut self) -> Result<bool, DomainError> {
        self.reader = None;
        if self.file_idx >= self.files.len() {
            return Ok(false);
        }
        let path = &self.files[self.file_idx];
        self.file_idx += 1;
        let file = File::open(path)
            .map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
        let meta = file
            .metadata()
            .map_err(|e| DomainError::Io(format!("metadata {}: {e}", path.display())))?;
        validate_vkl1_len(meta.len(), path)?;
        self.reader = Some(BufReader::new(file));
        Ok(true)
    }
}

impl CandleSeriesReader for StreamingVkl1Reader {
    fn next_candle(&mut self) -> Result<Option<Candle>, DomainError> {
        loop {
            if self.reader.is_none() && !self.open_next_file()? {
                return Ok(None);
            }
            let Some(reader) = self.reader.as_mut() else {
                return Ok(None);
            };
            let mut buf = [0_u8; VKL1_RECORD_BYTES];
            let got = read_exact_or_eof(reader, &mut buf)?;
            if got == 0 {
                self.reader = None;
                continue;
            }
            if got < VKL1_RECORD_BYTES {
                return Err(DomainError::FormatCorrupted(
                    "truncated vkl1 record".to_string(),
                ));
            }
            let row = decode_vkl1_record(&buf)?;
            return Ok(Some(row.to_candle()?));
        }
    }
}

fn validate_vkl1_len(len: u64, path: &Path) -> Result<(), DomainError> {
    if !len.is_multiple_of(VKL1_RECORD_BYTES as u64) {
        return Err(DomainError::FormatCorrupted(format!(
            "vkl1 file {} length {} not divisible by {}",
            path.display(),
            len,
            VKL1_RECORD_BYTES
        )));
    }
    Ok(())
}

fn collect_and_sort_vkl1_files(dir: &Path) -> Result<Vec<PathBuf>, DomainError> {
    let mut stack = vec![dir.to_path_buf()];
    let mut candidates = Vec::new();
    while let Some(cur) = stack.pop() {
        let entries = fs::read_dir(&cur)
            .map_err(|e| DomainError::Io(format!("read_dir {}: {e}", cur.display())))?;
        for entry_res in entries {
            let entry = entry_res
                .map_err(|e| DomainError::Io(format!("dir entry {}: {e}", cur.display())))?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("vkl1"))
            {
                let first_ts = probe_first_vkl1_ts(&p)?.unwrap_or(i64::MAX);
                candidates.push((first_ts, p));
            }
        }
    }
    if candidates.is_empty() {
        return Err(DomainError::Io(format!(
            "no .vkl1 files in {}",
            dir.display()
        )));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(candidates.into_iter().map(|(_, p)| p).collect())
}

fn probe_first_vkl1_ts(path: &Path) -> Result<Option<i64>, DomainError> {
    let file =
        File::open(path).map_err(|e| DomainError::Io(format!("open {}: {e}", path.display())))?;
    let meta = file
        .metadata()
        .map_err(|e| DomainError::Io(format!("metadata {}: {e}", path.display())))?;
    validate_vkl1_len(meta.len(), path)?;
    let mut reader = BufReader::new(file);
    let mut buf = [0_u8; VKL1_RECORD_BYTES];
    let got = read_exact_or_eof(&mut reader, &mut buf)?;
    if got == 0 {
        return Ok(None);
    }
    if got < VKL1_RECORD_BYTES {
        return Err(DomainError::FormatCorrupted(
            "truncated vkl1 record".to_string(),
        ));
    }
    let row = decode_vkl1_record(&buf)?;
    Ok(Some(row.open_ts_ns))
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<usize, DomainError> {
    let mut got = 0_usize;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(DomainError::Io(format!("vkl1 read error: {e}"))),
        }
    }
    Ok(got)
}
