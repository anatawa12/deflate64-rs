use deflate64::InflaterManaged;

static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/issue-13/unitwf-1.5.0.minimized.zip");
static BINARY_DATA: &[u8] = include_bytes!("../test-assets/issue-13/logo.png");

fn deflate64_data() -> &'static [u8] {
    &ZIP_FILE_DATA[1182..][..34919]
}

#[test]
fn issue_13() {
    let compressed_data = deflate64_data();

    let mut uncompressed_data = vec![0u8; BINARY_DATA.len()];

    let mut inflater = Box::new(InflaterManaged::new());
    let output = inflater.inflate(compressed_data, &mut uncompressed_data);

    assert_eq!(output.bytes_consumed, compressed_data.len());
    assert_eq!(output.bytes_written, uncompressed_data.len());
    assert!(!output.data_error, "unexpected error");

    assert_eq!(uncompressed_data.as_slice(), BINARY_DATA);
}
