#![cfg(feature = "checkpoint")]

use deflate64::{CheckpointStreamPositions, InflaterManaged};

const BINARY_WAV_DATA_OFFSET: usize = 40;
const BINARY_WAV_COMPRESSED_SIZE: usize = 2669743;
const BINARY_WAV_UNCOMPRESSED_SIZE: usize = 2703788;

static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/deflate64.zip");
static BINARY_WAV_DATA: &[u8] = include_bytes!("../test-assets/folder/binary.wmv");

fn compressed_data() -> &'static [u8] {
    &ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE]
}

fn assert_bytes_eq(actual: &[u8], expected: &[u8], msg: &str) {
    match actual.iter().zip(expected).position(|(a, b)| a != b) {
        Some(pos) => panic!(
            "{msg}: first diff at byte {pos}: got 0x{:02x}, expected 0x{:02x} (len {} vs {})",
            actual[pos],
            expected[pos],
            actual.len(),
            expected.len()
        ),
        None => {
            if actual.len() != expected.len() {
                panic!(
                    "{msg}: length mismatch: {} vs {}",
                    actual.len(),
                    expected.len()
                );
            }
        }
    }
}

fn inflate_with_checkpoints(output_interval: usize) -> Vec<(Vec<u8>, CheckpointStreamPositions)> {
    let mut inflater = Box::new(InflaterManaged::new());
    let mut output = vec![0u8; BINARY_WAV_UNCOMPRESSED_SIZE + 100];
    let mut written = 0;
    let mut consumed = 0;
    let mut checkpoints = Vec::new();
    let mut next_checkpoint_at = output_interval;
    let compressed = compressed_data();

    while !inflater.finished() {
        let out_end = next_checkpoint_at.max(written).min(output.len());
        let result = inflater.inflate(&compressed[consumed..], &mut output[written..out_end]);
        consumed += result.bytes_consumed;
        written += result.bytes_written;
        assert!(!result.data_error);

        if written >= next_checkpoint_at {
            if let Some((cp_data, positions)) = inflater.checkpoint() {
                assert_eq!(positions.output_bytes_already_returned, written as u64);
                checkpoints.push((cp_data, positions));
            }
            next_checkpoint_at += output_interval;
        }
    }

    assert_eq!(written, BINARY_WAV_UNCOMPRESSED_SIZE);
    assert_eq!(&output[..written], BINARY_WAV_DATA);
    checkpoints
}

fn resume_from_checkpoint(
    inflater: &mut InflaterManaged,
    compressed: &[u8],
    positions: &CheckpointStreamPositions,
) -> Vec<u8> {
    let input_skip = positions.input_bytes_to_skip as usize;
    let mut output = vec![0u8; BINARY_WAV_UNCOMPRESSED_SIZE + 100];
    let mut written = 0;
    let mut consumed = input_skip;

    while consumed < compressed.len() && !inflater.finished() {
        let r = inflater.inflate(&compressed[consumed..], &mut output[written..]);
        consumed += r.bytes_consumed;
        written += r.bytes_written;
        assert!(!r.data_error);
    }

    output.truncate(written);
    output
}

fn build_uncompressed_deflate_stream(data: &[u8]) -> Vec<u8> {
    assert!(data.len() <= 65535);
    let len = data.len() as u16;
    let mut stream = Vec::with_capacity(5 + data.len());
    stream.push(0b00000001); // BFINAL=1, BTYPE=00 (uncompressed)
    stream.extend_from_slice(&len.to_le_bytes());
    stream.extend_from_slice(&(!len).to_le_bytes());
    stream.extend_from_slice(data);
    stream
}

fn fletcher32_checksum(data: &[u8]) -> u32 {
    let (mut a, mut b) = (0u32, 0u32);
    for &byte in data {
        a = a.wrapping_add(byte as u32);
        b = b.wrapping_add(a);
    }
    (b << 16) | (a & 0xFFFF)
}

fn build_synthetic_checkpoint(window_data: &[u8]) -> Vec<u8> {
    let mut cp = vec![0u8; 346 + window_data.len()];
    cp[0..8].copy_from_slice(&1000u64.to_le_bytes()); // input_bits
    cp[8] = 0; // buffered_value
    cp[9] = 0; // block_type=Uncompressed, bfinal=0
    cp[10..14].copy_from_slice(&33333u32.to_le_bytes()); // uncompressed_remaining
    cp[14..334].fill(0xFF); // unused code lengths
    cp[334..342].copy_from_slice(&(window_data.len() as u64).to_le_bytes()); // output_bytes_written
    cp[342..346].copy_from_slice(&(window_data.len() as u32).to_le_bytes()); // output_bytes_unread
    cp[346..].fill(0xFE); // window_data
    let checksum = fletcher32_checksum(&cp);
    cp.extend_from_slice(&checksum.to_le_bytes());
    cp
}

#[test]
fn checkpoint_availability_lifecycle() {
    // Before first inflate
    let mut inflater = Box::new(InflaterManaged::new());
    assert!(inflater.checkpoint().is_none());

    // Mid-stream
    let mut output = vec![0u8; 1024];
    let result = inflater.inflate(&compressed_data()[..1000], &mut output);
    assert!(!inflater.finished() && !result.data_error);
    assert!(inflater.checkpoint().is_some());

    // After finished with output drained
    let mut inflater2 = Box::new(InflaterManaged::new());
    let mut output2 = vec![0u8; BINARY_WAV_UNCOMPRESSED_SIZE + 100];
    inflater2.inflate(compressed_data(), &mut output2);
    assert!(inflater2.finished());
    assert!(inflater2.checkpoint().is_none());
}

#[test]
fn failed_restore_preserves_inflater_state() {
    let checkpoints = inflate_with_checkpoints(10000);
    let (valid_cp, _) = &checkpoints[checkpoints.len() / 2];

    let mut inflater = Box::new(InflaterManaged::new());
    inflater.restore_from_checkpoint(valid_cp).unwrap();
    let (before, _) = inflater.checkpoint().unwrap();

    let mut bad_cp = build_synthetic_checkpoint(&vec![0; 65538]);
    let len = bad_cp.len();
    bad_cp[9] = 2; // block_type=Dynamic, bfinal=0
    bad_cp[10..14].fill(0); // uncompressed_remaining = 0
    bad_cp[14..334].fill(17); // invalid code length
    let checksum = fletcher32_checksum(&bad_cp[..len - 4]);
    bad_cp[len - 4..].copy_from_slice(&checksum.to_le_bytes());
    assert!(inflater.restore_from_checkpoint(&bad_cp).is_none());

    let (after, _) = inflater.checkpoint().unwrap();
    assert_bytes_eq(&before, &after, "state changed after failed restore");
}

#[test]
fn corrupted_checkpoint_rejected() {
    let checkpoints = inflate_with_checkpoints(10000);
    let (cp_data, _) = &checkpoints[checkpoints.len() / 2];
    let mut inflater = Box::new(InflaterManaged::new());

    let n = cp_data.len();
    for len in (0..=1000).chain(n - 10..=n - 1) {
        assert!(
            inflater.restore_from_checkpoint(&cp_data[..len]).is_none(),
            "len={len}"
        );
    }

    let mut corrupted = cp_data.clone();
    corrupted[100] ^= 0x01;
    assert!(inflater.restore_from_checkpoint(&corrupted).is_none());

    // Original still works
    assert!(inflater.restore_from_checkpoint(cp_data).is_some());
}

#[test]
fn restore_and_reserialize() {
    let checkpoints = inflate_with_checkpoints(10000);
    for (cp_data, cp_positions) in &checkpoints {
        let mut restored = Box::new(InflaterManaged::new());
        let positions = restored.restore_from_checkpoint(cp_data).unwrap();
        assert_eq!(cp_positions, &positions);

        let (reserialized, reser_pos) = restored.checkpoint().unwrap();
        assert_bytes_eq(cp_data, &reserialized, "checkpoint data");
        assert_eq!(cp_positions, &reser_pos);

        // Verify a small amount of forward decoder output matches expected
        let skip = positions.input_bytes_to_skip as usize;
        let out_skip = positions.output_bytes_already_returned as usize;
        let already_in_buffer = restored.available_output();
        if already_in_buffer + out_skip < BINARY_WAV_COMPRESSED_SIZE {
            let mut out = vec![0u8; already_in_buffer + 1000];
            let r = restored.inflate(&compressed_data()[skip..], &mut out);
            assert!(!r.data_error && r.bytes_written > already_in_buffer);
            assert_bytes_eq(
                &out[..r.bytes_written],
                &BINARY_WAV_DATA[out_skip..][..r.bytes_written],
                "output",
            );
        }
    }
}

#[test]
fn restore_continue_restore() {
    let checkpoints = inflate_with_checkpoints(10000);
    let mut inflater = Box::new(InflaterManaged::new());

    // Restore, do work
    let (cp1, _) = &checkpoints[0];
    let pos1 = inflater.restore_from_checkpoint(cp1).unwrap();
    let mut output = vec![0u8; 50000];
    inflater.inflate(
        &compressed_data()[pos1.input_bytes_to_skip as usize..],
        &mut output,
    );

    // Take new checkpoint, restore it, verify output
    let (new_cp, new_pos) = inflater.checkpoint().unwrap();
    let mut restored = Box::new(InflaterManaged::new());
    restored.restore_from_checkpoint(&new_cp).unwrap();
    let output = resume_from_checkpoint(&mut restored, compressed_data(), &new_pos);
    assert_bytes_eq(
        &output,
        &BINARY_WAV_DATA[new_pos.output_bytes_already_returned as usize..],
        "output",
    );
}

#[test]
fn checkpoint_uncompressed_block() {
    let original: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let compressed = build_uncompressed_deflate_stream(&original);

    // Decompress partway, take checkpoint
    let mut inflater = Box::new(InflaterManaged::new());
    let mut output = vec![0u8; 500];
    inflater.inflate(&compressed, &mut output);
    let (cp_data, positions) = inflater.checkpoint().unwrap();

    // Verify reserialization
    let mut restored = Box::new(InflaterManaged::new());
    restored.restore_from_checkpoint(&cp_data).unwrap();
    let (reserialized, _) = restored.checkpoint().unwrap();
    assert_bytes_eq(&cp_data, &reserialized, "reserialized checkpoint");

    // Verify restored output matches expected
    let skip = positions.input_bytes_to_skip as usize;
    let mut out = vec![0u8; 2000];
    let r = restored.inflate(&compressed[skip..], &mut out);
    let output_skip = positions.output_bytes_already_returned as usize;
    assert_bytes_eq(&out[..r.bytes_written], &original[output_skip..], "output");
}

#[test]
fn checkpoint_with_large_unread_buffer() {
    const MAX_HISTORY: usize = 65538; // deflate64 back-reference window

    // Synthetic stream producing 327681 zeros, exceeding output buffer
    let compressed: &[u8] = &[
        0x63, 0x18, 0xed, 0xff, 0x07, 0xa3, 0xfd, 0xff, 0x60, 0xb4, 0xff, 0x1f, 0x8c, 0xf6, 0xff,
        0x83, 0xd1, 0xfe, 0x7f, 0x00, 0x00,
    ];

    let mut inflater = InflaterManaged::new();
    let mut out = [0u8; 1];
    for _ in 0..3 {
        out[0] = 0xff;
        let r = inflater.inflate(compressed, &mut out);
        assert_eq!(out[0], 0);
        assert!(!r.data_error);
    }

    let avail_before = inflater.available_output();
    assert!(avail_before > MAX_HISTORY);

    let (cp, _) = inflater.checkpoint().unwrap();
    assert!(cp.len() > 346 + MAX_HISTORY);

    let mut restored = InflaterManaged::new();
    restored.restore_from_checkpoint(&cp).unwrap();
    assert_eq!(avail_before, restored.available_output());

    // Drain buffered output with no input
    let mut drained = vec![0xffu8; avail_before];
    let r = restored.inflate(&[], &mut drained);
    assert_eq!(r.bytes_written, avail_before);
    assert!(drained.iter().all(|&b| b == 0));
}

#[test]
fn all_window_bytes_restored_properly() {
    const OUTPUT_BUFFER_SIZE: usize = 262144;

    let checkpoints = inflate_with_checkpoints(10000);
    let (real_cp, real_positions) = &checkpoints[checkpoints.len() / 2];

    let mut inflater = Box::new(InflaterManaged::new());
    let synthetic_cp = build_synthetic_checkpoint(&vec![0xFE; OUTPUT_BUFFER_SIZE]);
    inflater.restore_from_checkpoint(&synthetic_cp).unwrap();
    assert_eq!(inflater.available_output(), OUTPUT_BUFFER_SIZE);

    // Drain synthetic buffered output to prove that we overwrite the entire window buffer
    let mut drained = vec![0u8; OUTPUT_BUFFER_SIZE];
    let r = inflater.inflate(&[], &mut drained);
    assert_eq!(r.bytes_written, OUTPUT_BUFFER_SIZE);
    assert!(drained.iter().all(|&b| b == 0xFE));

    // Restore real checkpoint and verify output to prove that we restored window correctly
    let restore_positions = inflater.restore_from_checkpoint(real_cp).unwrap();
    assert_eq!(real_positions, &restore_positions);

    let output = resume_from_checkpoint(&mut inflater, compressed_data(), &restore_positions);
    let output_skip = restore_positions.output_bytes_already_returned as usize;
    assert_bytes_eq(
        &output,
        &BINARY_WAV_DATA[output_skip..],
        "output after real restore",
    );
}
