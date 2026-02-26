use deflate64::Deflate64Decoder;
use std::io::{Cursor, Read};

static CASE1_ZIP_DATA: &[u8] = include_bytes!("../test-assets/forced_buffer_exhaustion.zip");

const PAYLOAD_OFFSET: usize = 39;

fn case1_data() -> &'static [u8] {
    &CASE1_ZIP_DATA[PAYLOAD_OFFSET..]
}

#[test]
fn forced_buffer_exhaustion() {
    // Forcefully truncate the stream to 10 bytes to trigger the fast-loop bailout
    // and the UnexpectedEof patch in stream.rs
    let truncated_data = &case1_data()[..10];
    let mut decoder = Deflate64Decoder::new(Cursor::new(truncated_data));
    let mut buf = Vec::new();

    let result = decoder.read_to_end(&mut buf);

    assert!(result.is_err(), "Expected an error due to truncated stream");
    assert_eq!(
        result.unwrap_err().kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}
