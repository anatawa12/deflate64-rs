mod input_buffer;
mod huffman_tree;
mod inflater_managed;
mod output_window;

#[derive(Copy, Clone, Eq, PartialEq)]
enum BlockType
{
    Uncompressed = 0,
    Static = 1,
    Dynamic = 2
}

impl BlockType {
    pub fn from_int(int: i32) -> Option<BlockType> {
        match int {
            0 => Some(Self::Uncompressed),
            1 => Some(Self::Static),
            2 => Some(Self::Dynamic),
            _ => None
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum InflaterState
{
    //ReadingHeader = 0,           // Only applies to GZIP

    ReadingBFinal = 2,
    // About to read bfinal bit
    ReadingBType = 3,                // About to read blockType bits

    ReadingNumLitCodes = 4,
    // About to read # literal codes
    ReadingNumDistCodes = 5,
    // About to read # dist codes
    ReadingNumCodeLengthCodes = 6,
    // About to read # code length codes
    ReadingCodeLengthCodes = 7,
    // In the middle of reading the code length codes
    ReadingTreeCodesBefore = 8,
    // In the middle of reading tree codes (loop top)
    ReadingTreeCodesAfter = 9,       // In the middle of reading tree codes (extension; code > 15)

    DecodeTop = 10,
    // About to decode a literal (char/match) in a compressed block
    HaveInitialLength = 11,
    // Decoding a match, have the literal code (base length)
    HaveFullLength = 12,
    // Ditto, now have the full match length (incl. extra length bits)
    HaveDistCode = 13,               // Ditto, now have the distance code also, need extra dist bits

    /* uncompressed blocks */
    UncompressedAligning = 15,
    UncompressedByte1 = 16,
    UncompressedByte2 = 17,
    UncompressedByte3 = 18,
    UncompressedByte4 = 19,
    DecodingUncompressed = 20,

    // These three apply only to GZIP
    //StartReadingFooter = 21,
    // (Initialisation for reading footer)
    //ReadingFooter = 22,
    //VerifyingFooter = 23,

    Done = 24 // Finished
}

impl std::ops::Sub for InflaterState {
    type Output = u8;

    fn sub(self, rhs: Self) -> Self::Output {
        self as u8 - rhs as u8
    }
}

fn array_copy<T : Copy>(source: &[T], dst: &mut [T], length: usize) {
    dst[..length].copy_from_slice(&source[..length]);
}

fn array_copy1<T : Copy>(source: &[T], source_index: usize, dst: &mut [T], dst_index: usize, length: usize) {
    dst[dst_index..][..length].copy_from_slice(&source[source_index..][..length]);
}

#[derive(Debug)]
pub struct InflateResult {
    /// The number of bytes consumed from the input slice.
    pub bytes_consumed: usize,
    /// The number of bytes written to the output slice.
    pub bytes_written: usize,
}

impl InflateResult {
    pub fn new() -> Self {
        Self {
            bytes_consumed: 0,
            bytes_written: 0,
        }
    }
}
