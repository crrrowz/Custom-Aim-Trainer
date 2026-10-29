pub mod format;
pub mod reader;
pub mod writer;

pub use format::*;
pub use reader::{
    delta_decode_u64, parse_footer_v1, read_aft, read_input_stream_selective, ReadError,
    FOOTER_LEN_V1,
};
pub use writer::{delta_encode_u64, write_aft, WriteError};

/// Constructs a canonical sample session for testing across crates and tools.
pub fn sample_session() -> AftSession {
    AftSession {
        header: AftHeader {
            session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
            created_at_utc: "2026-10-19T08:30:00Z".to_string(),
            app_version: "0.1.0".to_string(),
            trainer_version: "0.1.0".to_string(),
            scenario_id: "assess.v1.flick_static".to_string(),
            scenario_version: 1,
            form: Some(2),
            seed: 987654321,
            game_profile: "valorant.v1".to_string(),
            mouse: MouseConfig {
                dpi: 800,
                sens: 0.35,
                deg_per_count: 0.07 * 0.35,
            },
            display: DisplayConfig {
                width: 1920,
                height: 1080,
                hz: 240,
                hfov: 103.0,
                vfov: 70.53,
            },
            input: InputConfig {
                est_polling_hz: 1000,
            },
            thresholds_version: "thresholds.v1".to_string(),
            quality_flags: 0,
        },
        streams: AftStreams {
            input: InputStream {
                t_us: vec![1000, 2000, 3000, 4000, 5000],
                dx: vec![0, 1, 3, -1, 0],
                dy: vec![0, -1, 0, 2, 0],
            },
            frames: FrameStream {
                t_us: vec![4166, 8333, 12500],
                present_t_us: vec![4300, 8450, 12620],
            },
            view: ViewStream {
                t_us: vec![1000, 2000, 3000],
                yaw_deg: vec![0.0, 0.0245, 0.098],
                pitch_deg: vec![0.0, -0.0245, -0.0245],
            },
            targets: TargetStream {
                records: vec![TargetRecord {
                    id: 1,
                    spawn_t_us: 1000,
                    despawn_t_us: 450000,
                    kind: TargetKind::Sphere,
                    params: TargetParams {
                        radius_deg: 0.5,
                        initial_pos: [10.0, 0.0, 15.0],
                        path_spline: None,
                    },
                }],
            },
            shots: ShotStream {
                records: vec![ShotRecord {
                    t_us: 450000,
                    hit_target_id: Some(1),
                    hitzone: HitZone::Head,
                }],
            },
            keys: KeyStream {
                records: vec![
                    KeyRecord {
                        t_us: 2000,
                        key: KeyCode::KeyD,
                        down: true,
                    },
                    KeyRecord {
                        t_us: 250000,
                        key: KeyCode::KeyD,
                        down: false,
                    },
                ],
            },
            player: PlayerStream {
                t_us: vec![1000, 2000, 3000],
                vel_x: vec![0.0, 1.2, 5.4],
                vel_z: vec![0.0, 0.0, 0.0],
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_roundtrip_aft() {
        let original = sample_session();
        let bytes = write_aft(&original).expect("Serialization failed");
        assert!(bytes.len() >= FOOTER_LEN_V1 + 6);
        assert_eq!(&bytes[0..4], AFT_MAGIC);

        let decoded = read_aft(&bytes).expect("Deserialization failed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_selective_input_stream_read() {
        let original = sample_session();
        let bytes = write_aft(&original).expect("Serialization failed");

        let input_only =
            read_input_stream_selective(&bytes).expect("Selective input stream reading failed");
        assert_eq!(original.streams.input, input_only);
    }

    #[test]
    fn test_corrupted_crc_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");
        // Flip bit in payload
        bytes[10] ^= 0xAA;
        match read_aft(&bytes) {
            Err(ReadError::CrcMismatch { .. }) => (),
            other => panic!("Expected CRC mismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_footer_directory_length_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_dir_len_offset = bytes.len() - 6; // 2B directory length before 4B CRC
        bytes[footer_dir_len_offset] ^= 0x01;

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::FooterDirectoryLengthMismatch { .. }) => (),
            other => panic!("Expected FooterDirectoryLengthMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_truncated_file_rejected() {
        let original = sample_session();
        let bytes = write_aft(&original).expect("Serialization failed");

        // Severely truncated
        match read_aft(&bytes[0..40]) {
            Err(ReadError::Truncated) => (),
            other => panic!("Expected Truncated error, got {:?}", other),
        }
    }

    #[test]
    fn test_total_oversized_length_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_start = bytes.len() - FOOTER_LEN_V1;
        // Set stream 0 declared length to 64 MiB + 1
        let oversized = (MAX_TOTAL_UNCOMPRESSED_SIZE as u32) + 1;
        // Stream 0 entry is at footer_start + 12 (after 12B header entry), uncompressed_len at +8 inside entry
        let stream0_uncomp_offset = footer_start + 12 + 8;
        bytes[stream0_uncomp_offset..stream0_uncomp_offset + 4]
            .copy_from_slice(&oversized.to_le_bytes());

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::TotalOversizedLength { .. }) => (),
            other => panic!("Expected TotalOversizedLength, got {:?}", other),
        }
    }

    #[test]
    fn test_absurd_decompression_ratio_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_start = bytes.len() - FOOTER_LEN_V1;
        // Stream 0 entry compressed_len is bytes[footer_start + 16..footer_start + 20]
        let comp_len = u32::from_le_bytes(
            bytes[footer_start + 16..footer_start + 20]
                .try_into()
                .unwrap(),
        );
        // Set uncompressed_len > 1 MiB and > comp_len * 1000
        let absurd_uncomp = (comp_len * 1001).max(2 * 1024 * 1024);
        let stream0_uncomp_offset = footer_start + 20;
        bytes[stream0_uncomp_offset..stream0_uncomp_offset + 4]
            .copy_from_slice(&absurd_uncomp.to_le_bytes());

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::AbsurdCompressionRatio { .. }) => (),
            other => panic!("Expected AbsurdCompressionRatio, got {:?}", other),
        }
    }

    #[test]
    fn test_offset_inside_header_magic_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_start = bytes.len() - FOOTER_LEN_V1;
        // Set header offset to 4 (inside magic/version 0..6)
        let invalid_offset: u32 = 4;
        bytes[footer_start..footer_start + 4].copy_from_slice(&invalid_offset.to_le_bytes());

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::InvalidBlockOffset {
                offset,
                min_allowed,
            }) => {
                assert_eq!(offset, 4);
                assert_eq!(min_allowed, 6);
            }
            other => panic!("Expected InvalidBlockOffset, got {:?}", other),
        }
    }

    #[test]
    fn test_overlapping_blocks_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_start = bytes.len() - FOOTER_LEN_V1;
        // Header offset is at footer_start..footer_start+4
        let header_offset =
            u32::from_le_bytes(bytes[footer_start..footer_start + 4].try_into().unwrap());
        // Set stream 0 offset to equal header offset (exact collision/overlap)
        bytes[footer_start + 12..footer_start + 16].copy_from_slice(&header_offset.to_le_bytes());

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::OverlappingBlocks { .. }) => (),
            other => panic!("Expected OverlappingBlocks, got {:?}", other),
        }
    }

    #[test]
    fn test_oversized_declared_length_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        // Footer layout: header_entry uncompressed_len is at footer_start + 8
        let footer_start = bytes.len() - FOOTER_LEN_V1;
        let oversized = (MAX_HEADER_UNCOMPRESSED_SIZE as u32) + 1;
        bytes[footer_start + 8..footer_start + 12].copy_from_slice(&oversized.to_le_bytes());

        // Recompute valid CRC so it passes CRC check and hits the length guard
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::OversizedLength { declared, limit }) => {
                assert_eq!(declared, oversized as usize);
                assert_eq!(limit, MAX_HEADER_UNCOMPRESSED_SIZE);
            }
            other => panic!("Expected OversizedLength error, got {:?}", other),
        }
    }

    #[test]
    fn test_footer_length_mismatch_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        let footer_start = bytes.len() - FOOTER_LEN_V1;
        // Alter header declared uncompressed length by +1 byte (still under max limit)
        let original_len = u32::from_le_bytes(
            bytes[footer_start + 8..footer_start + 12]
                .try_into()
                .unwrap(),
        );
        let mismatched = original_len + 1;
        bytes[footer_start + 8..footer_start + 12].copy_from_slice(&mismatched.to_le_bytes());

        // Recompute valid CRC
        let crc_offset = bytes.len() - 4;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&bytes[..crc_offset]);
        let new_crc = hasher.finalize();
        bytes[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        match read_aft(&bytes) {
            Err(ReadError::LengthMismatch { .. }) => (),
            other => panic!("Expected LengthMismatch error, got {:?}", other),
        }
    }

    #[test]
    fn test_corrupted_zstd_payload_rejected() {
        let original = sample_session();
        let mut bytes = write_aft(&original).expect("Serialization failed");

        // Corrupt stream 0 payload bytes (between offset 6 and footer)
        bytes[15] ^= 0xFF;

        // Even if CRC were bypassed, decompress or postcard fails
        // With untouched CRC, it cleanly rejects with CrcMismatch
        assert!(read_aft(&bytes).is_err());
    }

    #[test]
    fn test_60s_session_file_size() {
        // Generate realistic 60s 1000Hz session (60,000 input events)
        let mut session = sample_session();
        let mut t_us = Vec::with_capacity(60_000);
        let mut dx = Vec::with_capacity(60_000);
        let mut dy = Vec::with_capacity(60_000);

        let mut current_t = 0u64;
        for i in 0..60_000 {
            current_t += 1000; // 1ms intervals
            t_us.push(current_t);
            dx.push((i % 7) - 3);
            dy.push((i % 5) - 2);
        }

        session.streams.input = InputStream { t_us, dx, dy };

        let encoded = write_aft(&session).expect("Failed writing 60s session");
        // Phase 0 Roadmap acceptance criterion: 60s file size <= 400KB
        assert!(
            encoded.len() <= 400 * 1024,
            "Encoded size {} bytes exceeded 400KB cap",
            encoded.len()
        );

        let roundtrip = read_aft(&encoded).expect("Failed reading 60s session");
        assert_eq!(
            session.streams.input.t_us.len(),
            roundtrip.streams.input.t_us.len()
        );
        assert_eq!(session.streams.input.dx, roundtrip.streams.input.dx);
        assert_eq!(session.streams.input.dy, roundtrip.streams.input.dy);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]
        #[test]
        fn test_fuzz_random_bytes_never_panics(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
            // Feeding arbitrary adversarial bytes must never panic, only return Err
            let _ = read_aft(&data);
        }
    }
}
