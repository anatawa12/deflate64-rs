use deflate64::InflaterManaged;
use std::cmp::min;

const BINARY_WAV_DATA_OFFSET: usize = 40;
const BINARY_WAV_COMPRESSED_SIZE: usize = 2669743;
const BINARY_WAV_UNCOMPRESSED_SIZE: usize = 2703788;
const BINARY_WAV_UNCOMPRESSED_BUFFER_SIZE: usize = 2703788 + 10;

static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/deflate64.zip");
static BINARY_WAV_DATA: &[u8] = include_bytes!("../test-assets/folder/binary.wmv");

#[test]
fn check_test_data() {
    assert_eq!(BINARY_WAV_DATA.len(), BINARY_WAV_UNCOMPRESSED_SIZE);
}

#[test]
fn binary_wav() {
    let binary_wav_compressed =
        &ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE];
    let mut uncompressed_data = vec![0u8; BINARY_WAV_UNCOMPRESSED_BUFFER_SIZE];

    let mut inflater = Box::new(InflaterManaged::new());
    let output = inflater.inflate(binary_wav_compressed, &mut uncompressed_data);
    assert_eq!(output.bytes_consumed, BINARY_WAV_COMPRESSED_SIZE);
    assert_eq!(output.bytes_written, BINARY_WAV_UNCOMPRESSED_SIZE);
    assert!(!output.data_error, "unexpected error");

    assert_eq!(
        &uncompressed_data[..BINARY_WAV_UNCOMPRESSED_SIZE],
        BINARY_WAV_DATA
    );
}

#[test]
fn binary_wav_with_size() {
    let binary_wav_compressed =
        &ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE];
    let mut uncompressed_data = vec![0u8; BINARY_WAV_UNCOMPRESSED_BUFFER_SIZE];

    let mut inflater = Box::new(InflaterManaged::with_uncompressed_size(
        BINARY_WAV_UNCOMPRESSED_SIZE,
    ));
    let output = inflater.inflate(binary_wav_compressed, &mut uncompressed_data);
    assert_eq!(output.bytes_consumed, BINARY_WAV_COMPRESSED_SIZE);
    assert_eq!(output.bytes_written, BINARY_WAV_UNCOMPRESSED_SIZE);
    assert!(!output.data_error, "unexpected error");

    assert_eq!(
        &uncompressed_data[..BINARY_WAV_UNCOMPRESSED_SIZE],
        BINARY_WAV_DATA
    );
}

#[test]
fn binary_wav_shredded_1() {
    binary_wav_shredded(1)
}

#[test]
fn binary_wav_shredded_10() {
    binary_wav_shredded(10)
}

#[test]
fn binary_wav_shredded_100() {
    binary_wav_shredded(100)
}

fn binary_wav_shredded(chunk: usize) {
    let binary_wav_compressed =
        &ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE];
    let mut uncompressed_data = vec![0u8; BINARY_WAV_UNCOMPRESSED_BUFFER_SIZE];

    let mut inflater = Box::new(InflaterManaged::new());

    let mut compressed = binary_wav_compressed;
    let mut written = 0;

    while !compressed.is_empty() {
        let output = inflater.inflate(
            &compressed[..min(chunk, compressed.len())],
            &mut uncompressed_data[written..],
        );
        compressed = &compressed[output.bytes_consumed..];
        written += output.bytes_written;
        assert!(!output.data_error, "unexpected error");
    }

    assert_eq!(written, BINARY_WAV_UNCOMPRESSED_SIZE);

    assert_eq!(
        &uncompressed_data[..BINARY_WAV_UNCOMPRESSED_SIZE],
        BINARY_WAV_DATA
    );
}
