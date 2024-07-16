use std::io::{Cursor, Read};

use deflate64::Deflate64Decoder;

static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/issue-29/raw.zip");

fn deflate64_data() -> &'static [u8] {
    &ZIP_FILE_DATA[121..]
}

// panic with invalid deflate64 data (too big lookup length)

#[test]
fn issue_29() {
    let mut file = Deflate64Decoder::new(Cursor::new(deflate64_data()));
    let mut buf = Vec::new();
    let _ = file.read(&mut buf);
}

static VALID_ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/deflate64.zip");
const BINARY_WAV_DATA_OFFSET: usize = 40;
const BINARY_WAV_COMPRESSED_SIZE: usize = 2669743;

fn valid_zip_source_stream() -> &'static [u8] {
    &VALID_ZIP_FILE_DATA[BINARY_WAV_DATA_OFFSET..][..BINARY_WAV_COMPRESSED_SIZE]
}
#[test]
fn binary_wav() {
    let mut decoder = Deflate64Decoder::new(Cursor::new(valid_zip_source_stream()));
    let mut output = [];
    let read = decoder.read(&mut output).unwrap();
    assert_eq!(read, 0);
}
