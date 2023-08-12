use deflate64::Deflate64Decoder;
use std::io::{Cursor, Read};

const BINARY_WAV_DATA_OFFSET: usize = 40;
const BINARY_WAV_COMPRESSED_SIZE: usize = 2669743;
const BINARY_WAV_UNCOMPRESSED_SIZE: usize = 2703788;

static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/deflate64.zip");
static BINARY_WAV_DATA: &[u8] = include_bytes!("../test-assets/folder/binary.wmv");

#[test]
fn check_test_data() {
    assert_eq!(BINARY_WAV_DATA.len(), BINARY_WAV_UNCOMPRESSED_SIZE);
}

fn source_stream() -> &'static [u8] {
    &ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE]
}

#[test]
fn decode_from_read() {
    let mut decoder = Deflate64Decoder::new(Cursor::new(source_stream()));

    let mut uncompressed_data = vec![];
    decoder.read_to_end(&mut uncompressed_data).unwrap();

    assert_eq!(&uncompressed_data[..], BINARY_WAV_DATA);
}

#[test]
fn decode_from_buf_read() {
    let mut decoder = Deflate64Decoder::with_buffer(Cursor::new(source_stream()));

    let mut uncompressed_data = vec![];
    decoder.read_to_end(&mut uncompressed_data).unwrap();

    assert_eq!(&uncompressed_data[..], BINARY_WAV_DATA);
}
