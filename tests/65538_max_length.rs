use deflate64::Deflate64Decoder;
use std::io::{Cursor, Read};

static CASE2_ZIP_DATA: &[u8] = include_bytes!("../test-assets/65538_max_length.zip");

const PAYLOAD_OFFSET: usize = 39;

fn case2_data() -> &'static [u8] {
    &CASE2_ZIP_DATA[PAYLOAD_OFFSET..]
}

#[test]
fn max_length_truncation() {
    // This stream contains a single match length of 65538.
    let mut decoder = Deflate64Decoder::new(Cursor::new(case2_data()));
    let mut buf = Vec::new();

    let result = decoder.read_to_end(&mut buf);

    assert!(result.is_ok(), "Stream rejected valid max-length lookup");

    let bytes_read = result.unwrap();
    assert_eq!(bytes_read, 65539, "Decompressed size mismatch");

    // The hand-crafted payload actually resolves to `\0` null bytes
    assert!(
        buf.iter().all(|&b| b == 0x00),
        "Decompressed data is corrupted"
    );
}
