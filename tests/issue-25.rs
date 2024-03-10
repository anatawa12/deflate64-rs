use deflate64::InflaterManaged;

static ZIP_FILE_DATA: &[u8] =
    include_bytes!("../test-assets/issue-25/deflate64_not_enough_space.zip");

fn deflate64_data() -> &'static [u8] {
    &ZIP_FILE_DATA[30..]
}

// panic with invalid deflate64 data (too big lookup length)

#[test]
fn issue_25() {
    let compressed_data = deflate64_data();

    let mut inflater = Box::new(InflaterManaged::new());
    let mut sink = vec![0u8; 1024 * 1024 * 4];

    let output = inflater.inflate(compressed_data, &mut sink);

    assert!(
        output.data_error,
        "expected an error since this deflate64 file is invalid"
    );
}
