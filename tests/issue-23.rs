use deflate64::InflaterManaged;

static DEFLATE64_DATA: &[u8] = include_bytes!("../test-assets/issue-23/raw_deflate64_index_out_of_bounds");

#[test]
fn issue_23() {
    let compressed_data = DEFLATE64_DATA;

    let mut inflater = Box::new(InflaterManaged::new());
    let output = inflater.inflate(compressed_data, &mut vec![0u8; 1024]);
    assert!(output.data_error, "expected an error since this deflate64 file is invalid");
}
