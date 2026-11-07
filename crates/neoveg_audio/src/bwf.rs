//! Broadcast Wave metadata: the `bext` chunk of a RIFF/WAVE file (EBU Tech 3285).
//!
//! Decoding does not need this (a WAV with a `bext` chunk decodes like any other); this reads what
//! the chunk says about the take: its description, who made it, when it was recorded and the time
//! code of its first sample.

/// The fields of a `bext` chunk. Text fields are trimmed of the padding NULs and spaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bext {
    pub description: String,
    pub originator: String,
    pub originator_reference: String,
    /// `yyyy-mm-dd`, as written in the file.
    pub origination_date: String,
    /// `hh:mm:ss`, as written in the file.
    pub origination_time: String,
    /// Samples since midnight of the first sample.
    pub time_reference: u64,
    pub version: u16,
}

/// The `bext` chunk of a WAV file, or `Ok(None)` when the file has none. `Err` when the file is not
/// a RIFF/WAVE file or the chunk is shorter than the standard's 602 bytes.
pub fn read_bext(bytes: &[u8]) -> Result<Option<Bext>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("não é um arquivo WAVE".into());
    }
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        let body_start = at + 8;
        let body_end = body_start.saturating_add(size).min(bytes.len());
        if id == b"bext" {
            return parse(&bytes[body_start..body_end]).map(Some);
        }
        // Chunks are word-aligned: an odd size has one pad byte
        at = body_start.saturating_add(size + (size & 1));
    }
    Ok(None)
}

fn parse(b: &[u8]) -> Result<Bext, String> {
    if b.len() < 602 {
        return Err(format!("chunk bext curto demais: {} bytes", b.len()));
    }
    let text = |from: usize, len: usize| -> String {
        String::from_utf8_lossy(&b[from..from + len])
            .trim_matches(|c: char| c == '\0' || c == ' ')
            .to_string()
    };
    Ok(Bext {
        description: text(0, 256),
        originator: text(256, 32),
        originator_reference: text(288, 32),
        origination_date: text(320, 10),
        origination_time: text(330, 8),
        time_reference: u64::from(u32::from_le_bytes([b[338], b[339], b[340], b[341]]))
            | (u64::from(u32::from_le_bytes([b[342], b[343], b[344], b[345]])) << 32),
        version: u16::from_le_bytes([b[346], b[347]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_with_chunks(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut body = b"WAVE".to_vec();
        for (id, data) in chunks {
            body.extend_from_slice(*id);
            body.extend_from_slice(&(data.len() as u32).to_le_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut file = b"RIFF".to_vec();
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&body);
        file
    }

    fn bext_chunk(description: &str, time_reference: u64) -> Vec<u8> {
        let mut b = vec![0u8; 602];
        b[..description.len()].copy_from_slice(description.as_bytes());
        b[256..256 + 6].copy_from_slice(b"Neoveg");
        b[320..330].copy_from_slice(b"2026-11-07");
        b[330..338].copy_from_slice(b"18:30:00");
        b[338..342].copy_from_slice(&(time_reference as u32).to_le_bytes());
        b[342..346].copy_from_slice(&((time_reference >> 32) as u32).to_le_bytes());
        b[346..348].copy_from_slice(&1u16.to_le_bytes());
        b
    }

    #[test]
    fn the_fields_of_a_bext_chunk_come_out_trimmed() {
        let file = wav_with_chunks(&[
            (b"fmt ", vec![0; 16]),
            (b"bext", bext_chunk("Take 3", 48_000 * 3600)),
        ]);
        let got = read_bext(&file).unwrap().expect("has bext");
        assert_eq!(got.description, "Take 3");
        assert_eq!(got.originator, "Neoveg");
        assert_eq!(got.origination_date, "2026-11-07");
        assert_eq!(got.origination_time, "18:30:00");
        assert_eq!(got.time_reference, 48_000 * 3600);
        assert_eq!(got.version, 1);
    }

    #[test]
    fn a_plain_wav_has_no_bext_and_a_short_or_foreign_file_is_refused() {
        let plain = wav_with_chunks(&[(b"fmt ", vec![0; 16]), (b"data", vec![0; 4])]);
        assert_eq!(read_bext(&plain), Ok(None));
        assert!(read_bext(b"not a wave file at all").is_err());
        let short = wav_with_chunks(&[(b"bext", vec![0; 100])]);
        assert!(read_bext(&short).is_err());
    }

    #[test]
    fn an_odd_sized_chunk_before_bext_is_skipped_with_its_pad_byte() {
        let file = wav_with_chunks(&[
            (b"LIST", vec![1, 2, 3]),
            (b"bext", bext_chunk("after pad", 0)),
        ]);
        assert_eq!(read_bext(&file).unwrap().unwrap().description, "after pad");
    }
}
