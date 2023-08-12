#![cfg(feature = "__test-7zip")]
//! This test compresses some random data with deflate64 using p7zip `7z` command and check decompression

use std::ffi::OsString;
use std::fs::File;
use deflate64::Deflate64Decoder;
use std::io::{Cursor, Read, Write};
use std::mem::size_of;
use std::process::{Command, Stdio};
use proptest::proptest;
use tempfile::TempDir;

const TEST_FILE_NAME: &'static str = "test.file";
const TEST_ZIP_NAME: &'static str = "test.zip";

#[repr(packed)]
struct ZipLocalFileHeader {
    signature: [u8; 4],
    version: [u8; 2],
    flags: [u8; 2],
    compression_method: [u8; 2],
    last_mod_time: [u8; 2],
    last_mod_date: [u8; 2],
    uncompressed_crc32: [u8; 4],
    compressed_size: [u8; 4],
    uncompressed_size: [u8; 4],
    file_name_len: [u8; 2],
    extra_field_len: [u8; 2],
}

impl ZipLocalFileHeader {
    const SIGNATURE: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

    fn zero() -> Self {
        unsafe { std::mem::zeroed() }
    }

    fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self as *mut Self as *mut u8,
                size_of::<Self>()
            )
        }
    }
}

fn compress_with_7zip(data: &[u8]) -> Vec<u8> {
    let mut temp_dir = TempDir::new().unwrap();

    // write data to test file
    File::create(temp_dir.path().join(TEST_FILE_NAME)).unwrap()
        .write_all(data).unwrap();

    let seven_zip = std::env::var_os("SEVEN_ZIP_PATH").unwrap_or_else(|| OsString::from("7z"));

    let mut seven_zip_process = Command::new(seven_zip)
        .arg("a")
        .arg("-mm=Deflate64")
        .arg(TEST_ZIP_NAME)
        .arg(TEST_FILE_NAME)
        .current_dir(temp_dir.path())
        .output()
        .unwrap();

    if !seven_zip_process.status.success() {
        panic!(
            "7zip failure.\nstdout:\n{stdout}\n\nstderr:\n{stderr}",
               stdout = String::from_utf8(seven_zip_process.stdout).unwrap(),
               stderr = String::from_utf8(seven_zip_process.stderr).unwrap(),
        );
    }

    // parse zip file
    let mut zip_file = File::open(temp_dir.path().join(TEST_ZIP_NAME)).unwrap();
    let mut header = ZipLocalFileHeader::zero();
    zip_file.read_exact(header.as_mut_bytes()).unwrap();
    assert_eq!(header.signature, ZipLocalFileHeader::SIGNATURE);
    assert_eq!(u16::from_le_bytes(header.flags), 0);
    assert_eq!(u16::from_le_bytes(header.compression_method), 9);
    let compressed_size = u32::from_le_bytes(header.compressed_size);
    assert_eq!(u32::from_le_bytes(header.uncompressed_size) as usize, data.len());
    let file_name_size = u16::from_le_bytes(header.file_name_len);
    let extra_field_size = u16::from_le_bytes(header.extra_field_len);
    assert_eq!(file_name_size as usize, TEST_FILE_NAME.len());
    let mut file_name_buffer = vec![0u8; file_name_size as usize];
    let mut extra_field_buffer = vec![0u8; extra_field_size as usize];
    zip_file.read_exact(&mut file_name_buffer).unwrap();
    zip_file.read_exact(&mut extra_field_buffer).unwrap();
    assert_eq!(&file_name_buffer[..], TEST_FILE_NAME.as_bytes());
    drop((file_name_buffer, extra_field_buffer));

    let mut compressed_buffer = vec![0u8; compressed_size as usize];
    zip_file.read_exact(&mut compressed_buffer).unwrap();

    compressed_buffer
}

proptest! {
    #[test]
    fn decompress_compreesed_with_7zip(source_data in "\\PC{1000,}") {
        let source_data = source_data.as_bytes();
        let compressed = compress_with_7zip(&source_data);
        
        let mut decoder = Deflate64Decoder::new(Cursor::new(compressed));

        let mut uncompressed_data = vec![];
        decoder.read_to_end(&mut uncompressed_data).unwrap();

        assert_eq!(&uncompressed_data[..], source_data);
    }
}
