//! Deserializer, integrity validator, and selective stream reader for the .aft format (v1).

use crate::format::{
    AftFooter, AftHeader, AftSession, AftStreams, FrameStream, InputStream, KeyStream,
    PlayerStream, ShotStream, StreamIndexEntry, TargetStream, ViewStream, AFT_FORMAT_VERSION_V1,
    AFT_MAGIC, MAX_HEADER_UNCOMPRESSED_SIZE, MAX_STREAM_UNCOMPRESSED_SIZE, NUM_STREAMS_V1,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid magic signature: expected b\"AFT\\0\"")]
    InvalidMagic,
    #[error("Unsupported format version: {0}")]
    UnsupportedVersion(u16),
    #[error("Payload too short or truncated")]
    Truncated,
    #[error("CRC32 mismatch: expected {expected:#010x}, calculated {calculated:#010x}")]
    CrcMismatch { expected: u32, calculated: u32 },
    #[error("Postcard deserialization error: {0}")]
    Postcard(#[from] postcard::Error),
    #[error("Decompression error: {0}")]
    Decompress(std::io::Error),
    #[error("Declared uncompressed size {declared} exceeds security limit {limit}")]
    OversizedLength { declared: usize, limit: usize },
    #[error("Decompressed size mismatch: declared {declared}, got {actual}")]
    LengthMismatch { declared: usize, actual: usize },
    #[error("Footer directory length mismatch: expected {expected}, got {actual}")]
    FooterDirectoryLengthMismatch { expected: u16, actual: u16 },
    #[error("Integer overflow in buffer arithmetic")]
    Overflow,
}

/// Delta decode a slice of u64 deltas: [t0, Δ1, Δ2, ...] -> [t0, t0 + Δ1, t0 + Δ1 + Δ2, ...]
pub fn delta_decode_u64(deltas: &[u64]) -> Vec<u64> {
    if deltas.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(deltas.len());
    let mut current = deltas[0];
    out.push(current);
    for &delta in &deltas[1..] {
        current = current.wrapping_add(delta);
        out.push(current);
    }
    out
}

/// Fixed footer length for v1:
/// - Header entry: 12B
/// - 7 Stream entries: 84B
/// - Footer directory length: 2B
/// - CRC32: 4B
///
/// Total: 102 bytes
pub const FOOTER_LEN_V1: usize = 12 + (NUM_STREAMS_V1 * 12) + 2 + 4;

/// Decode a bounded, compressed block with decompression bomb prevention.
fn decode_block<T: serde::de::DeserializeOwned>(
    data: &[u8],
    entry: &StreamIndexEntry,
    limit: usize,
    payload_end: usize,
) -> Result<T, ReadError> {
    let uncompressed_len = entry.uncompressed_len as usize;
    if uncompressed_len > limit {
        return Err(ReadError::OversizedLength {
            declared: uncompressed_len,
            limit,
        });
    }

    let offset = entry.offset as usize;
    let compressed_len = entry.compressed_len as usize;

    let block_end = offset
        .checked_add(compressed_len)
        .ok_or(ReadError::Overflow)?;

    if block_end > payload_end {
        return Err(ReadError::Truncated);
    }

    let compressed_bytes = &data[offset..block_end];
    let decompressed = zstd::bulk::decompress(compressed_bytes, uncompressed_len)
        .map_err(ReadError::Decompress)?;

    if decompressed.len() != uncompressed_len {
        return Err(ReadError::LengthMismatch {
            declared: uncompressed_len,
            actual: decompressed.len(),
        });
    }

    let item: T = postcard::from_bytes(&decompressed)?;
    Ok(item)
}

/// Parse and validate the v1 footer table.
pub fn parse_footer_v1(data: &[u8]) -> Result<(AftFooter, usize), ReadError> {
    if data.len() < FOOTER_LEN_V1 + 6 {
        return Err(ReadError::Truncated);
    }

    // CRC validation
    let crc_offset = data.len() - 4;
    let expected_crc = u32::from_le_bytes(data[crc_offset..].try_into().unwrap());

    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&data[..crc_offset]);
    let calculated_crc = hasher.finalize();

    if expected_crc != calculated_crc {
        return Err(ReadError::CrcMismatch {
            expected: expected_crc,
            calculated: calculated_crc,
        });
    }

    let footer_start = data.len() - FOOTER_LEN_V1;
    let mut cursor = footer_start;

    // Header entry
    let h_offset = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
    let h_comp = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().unwrap());
    let h_uncomp = u32::from_le_bytes(data[cursor + 8..cursor + 12].try_into().unwrap());
    cursor += 12;

    let header_entry = StreamIndexEntry {
        offset: h_offset,
        compressed_len: h_comp,
        uncompressed_len: h_uncomp,
    };

    // 7 Stream entries
    let mut stream_entries = [StreamIndexEntry {
        offset: 0,
        compressed_len: 0,
        uncompressed_len: 0,
    }; NUM_STREAMS_V1];

    for entry in &mut stream_entries {
        let off = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
        let comp = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().unwrap());
        let uncomp = u32::from_le_bytes(data[cursor + 8..cursor + 12].try_into().unwrap());
        cursor += 12;

        *entry = StreamIndexEntry {
            offset: off,
            compressed_len: comp,
            uncompressed_len: uncomp,
        };
    }

    // Validate 2-byte footer directory length field (covers 12B header entry + 84B stream entries = 96B)
    let declared_footer_len = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap());
    let expected_footer_len = (12 + (NUM_STREAMS_V1 * 12)) as u16; // 96 bytes
    if declared_footer_len != expected_footer_len {
        return Err(ReadError::FooterDirectoryLengthMismatch {
            expected: expected_footer_len,
            actual: declared_footer_len,
        });
    }

    let footer = AftFooter {
        header_entry,
        stream_entries,
        crc32: expected_crc,
    };

    Ok((footer, footer_start))
}

/// Read and validate an AftSession from binary .aft data.
pub fn read_aft(data: &[u8]) -> Result<AftSession, ReadError> {
    if data.len() < 6 {
        return Err(ReadError::Truncated);
    }

    if &data[0..4] != AFT_MAGIC {
        return Err(ReadError::InvalidMagic);
    }

    let version = u16::from_le_bytes([data[4], data[5]]);
    match version {
        AFT_FORMAT_VERSION_V1 => read_aft_v1(data),
        v => Err(ReadError::UnsupportedVersion(v)),
    }
}

/// Version 1 reader implementation.
fn read_aft_v1(data: &[u8]) -> Result<AftSession, ReadError> {
    let (footer, footer_start) = parse_footer_v1(data)?;

    // 1. Decode header
    let header: AftHeader = decode_block(
        data,
        &footer.header_entry,
        MAX_HEADER_UNCOMPRESSED_SIZE,
        footer_start,
    )?;

    // 2. Decode Stream 0: Input
    let mut input: InputStream = decode_block(
        data,
        &footer.stream_entries[0],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;
    input.t_us = delta_decode_u64(&input.t_us);

    // 3. Decode Stream 1: Frames
    let mut frames: FrameStream = decode_block(
        data,
        &footer.stream_entries[1],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;
    frames.t_us = delta_decode_u64(&frames.t_us);
    frames.present_t_us = delta_decode_u64(&frames.present_t_us);

    // 4. Decode Stream 2: View
    let mut view: ViewStream = decode_block(
        data,
        &footer.stream_entries[2],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;
    view.t_us = delta_decode_u64(&view.t_us);

    // 5. Decode Stream 3: Targets
    let targets: TargetStream = decode_block(
        data,
        &footer.stream_entries[3],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;

    // 6. Decode Stream 4: Shots
    let shots: ShotStream = decode_block(
        data,
        &footer.stream_entries[4],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;

    // 7. Decode Stream 5: Keys
    let keys: KeyStream = decode_block(
        data,
        &footer.stream_entries[5],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;

    // 8. Decode Stream 6: Player
    let mut player: PlayerStream = decode_block(
        data,
        &footer.stream_entries[6],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;
    player.t_us = delta_decode_u64(&player.t_us);

    Ok(AftSession {
        header,
        streams: AftStreams {
            input,
            frames,
            view,
            targets,
            shots,
            keys,
            player,
        },
    })
}

/// Selectively read ONLY the InputStream from an .aft file without decoding other streams.
pub fn read_input_stream_selective(data: &[u8]) -> Result<InputStream, ReadError> {
    if data.len() < 6 {
        return Err(ReadError::Truncated);
    }
    if &data[0..4] != AFT_MAGIC {
        return Err(ReadError::InvalidMagic);
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version != AFT_FORMAT_VERSION_V1 {
        return Err(ReadError::UnsupportedVersion(version));
    }

    let (footer, footer_start) = parse_footer_v1(data)?;
    let mut input: InputStream = decode_block(
        data,
        &footer.stream_entries[0],
        MAX_STREAM_UNCOMPRESSED_SIZE,
        footer_start,
    )?;
    input.t_us = delta_decode_u64(&input.t_us);
    Ok(input)
}
