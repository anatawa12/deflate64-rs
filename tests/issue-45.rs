
// unexpected error with 65538 match length
// (previous version limits 65536 but actually 65538)

#[test]
fn issue_45() {
    use std::io::Read;
    let reader = std::io::Cursor::new([0xeb, 0x1f, 0xfd, 0xff, 0x07, 0x00]);
    let mut a = deflate64::Deflate64Decoder::with_buffer(reader);
    let mut buf = Vec::new();
    let b = a.read_to_end(&mut buf);
    assert!(b.is_ok());
}
