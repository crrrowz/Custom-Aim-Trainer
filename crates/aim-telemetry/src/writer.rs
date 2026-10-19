//! Serializer for the .aft format (v1).

use crate::format::{AftSession, StreamIndexEntry, AFT_FORMAT_VERSION_V1, AFT_MAGIC};
use std::io::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Postcard serialization error: {0}")]
    Postcard(#[from] postcard::Error),
    #[error("Offset overflow: stream offset exceeds 4GB limit")]
    OffsetOverflow,
}

/// Delta encode a slice of u64 timestamps: [t0, t1, t2, ...] -> [t0, t1 - t0, t2 - t1, ...]
pub fn delta_encode_u64(values: &[u64]) -> Vec<u64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(values.len());
    out.push(values[0]);
    for window in values.windows(2) {
        out.push(window[1].wrapping_sub(window[0]));
    }
    out
}

fn write_compressed_block<T: serde::Serialize>(
    buffer: &mut Vec<u8>,
    item: &T,
) -> Result<StreamIndexEntry, WriteError> {
    let raw = postcard::to_allocvec(item)?;
    let uncompressed_len = raw.len() as u32;
    let compressed = zstd::encode_all(&raw[..], 3)?;
    let compressed_len = compressed.len() as u32;

    let offset = buffer
        .len()
        .try_into()
        .map_err(|_| WriteError::OffsetOverflow)?;

    buffer.write_all(&compressed)?;

    Ok(StreamIndexEntry {
        offset,
        compressed_len,
        uncompressed_len,
    })
}

/// Write an AftSession into a byte vector implementing the .aft v1 binary specification:
/// 1. Magic (4B): `AFT\0`
/// 2. Version (2B): `1u16`
/// 3. Per-stream compressed blocks (Header, Input, Frames, View, Targets, Shots, Keys, Player)
/// 4. Footer directory:
///    - Header StreamIndexEntry (12B)
///    - 7 StreamIndexEntry records (84B)
///    - Footer directory length (2B)
///    - CRC32 of entire preceding payload (4B)
pub fn write_aft(session: &AftSession) -> Result<Vec<u8>, WriteError> {
    let mut buffer = Vec::new();

    // 1. Magic
    buffer.write_all(AFT_MAGIC)?;

    // 2. Format version
    buffer.write_all(&AFT_FORMAT_VERSION_V1.to_le_bytes())?;

    // 3. Compress and write header block
    let header_entry = write_compressed_block(&mut buffer, &session.header)?;

    // 4. Compress and write each stream independently
    // Stream 0: Input (delta-encoded timestamps)
    let mut input_storage = session.streams.input.clone();
    input_storage.t_us = delta_encode_u64(&input_storage.t_us);
    let entry_input = write_compressed_block(&mut buffer, &input_storage)?;

    // Stream 1: Frames (delta-encoded timestamps)
    let mut frames_storage = session.streams.frames.clone();
    frames_storage.t_us = delta_encode_u64(&frames_storage.t_us);
    frames_storage.present_t_us = delta_encode_u64(&frames_storage.present_t_us);
    let entry_frames = write_compressed_block(&mut buffer, &frames_storage)?;

    // Stream 2: View (delta-encoded timestamps, raw f64 angles)
    let mut view_storage = session.streams.view.clone();
    view_storage.t_us = delta_encode_u64(&view_storage.t_us);
    let entry_view = write_compressed_block(&mut buffer, &view_storage)?;

    // Stream 3: Targets
    let entry_targets = write_compressed_block(&mut buffer, &session.streams.targets)?;

    // Stream 4: Shots
    let entry_shots = write_compressed_block(&mut buffer, &session.streams.shots)?;

    // Stream 5: Keys
    let entry_keys = write_compressed_block(&mut buffer, &session.streams.keys)?;

    // Stream 6: Player kinematics
    let mut player_storage = session.streams.player.clone();
    player_storage.t_us = delta_encode_u64(&player_storage.t_us);
    let entry_player = write_compressed_block(&mut buffer, &player_storage)?;

    let stream_entries = [
        entry_input,
        entry_frames,
        entry_view,
        entry_targets,
        entry_shots,
        entry_keys,
        entry_player,
    ];

    // 5. Append Footer
    let footer_start = buffer.len();

    // Header entry: offset (4B), comp_len (4B), uncomp_len (4B)
    buffer.write_all(&header_entry.offset.to_le_bytes())?;
    buffer.write_all(&header_entry.compressed_len.to_le_bytes())?;
    buffer.write_all(&header_entry.uncompressed_len.to_le_bytes())?;

    // 7 Stream entries: 7 * 12 = 84B
    for entry in &stream_entries {
        buffer.write_all(&entry.offset.to_le_bytes())?;
        buffer.write_all(&entry.compressed_len.to_le_bytes())?;
        buffer.write_all(&entry.uncompressed_len.to_le_bytes())?;
    }

    let footer_len = (buffer.len() - footer_start) as u16;
    buffer.write_all(&footer_len.to_le_bytes())?;

    // Compute CRC32 of everything up to this point
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&buffer);
    let crc = hasher.finalize();

    buffer.write_all(&crc.to_le_bytes())?;

    Ok(buffer)
}
