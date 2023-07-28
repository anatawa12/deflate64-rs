use std::cmp::min;
use crate::*;
use crate::huffman_tree::HuffmanTree;
use crate::input_buffer::{BitsBuffer, InputBuffer};
use crate::output_window::OutputWindow;

// Extra bits for length code 257 - 285.
static EXTRA_LENGTH_BITS: &'static [u8] = &[
0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3,
3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 16
];

// The base length for length code 257 - 285.
// The formula to get the real length for a length code is lengthBase[code - 257] + (value stored in extraBits)
static LENGTH_BASE: &'static [u8] = &[
3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51,
59, 67, 83, 99, 115, 131, 163, 195, 227, 3
];

// The base distance for distance code 0 - 31
// The real distance for a distance code is  distanceBasePosition[code] + (value stored in extraBits)
static DISTANCE_BASE_POSITION: &'static [u16] = &[
1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 32769, 49153
];

// code lengths for code length alphabet is stored in following order
static CODE_ORDER: &'static [u8] = &[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

static STATIC_DISTANCE_TREE_TABLE: &'static [u8] = &[
0x00, 0x10, 0x08, 0x18, 0x04, 0x14, 0x0c, 0x1c, 0x02, 0x12, 0x0a, 0x1a,
0x06, 0x16, 0x0e, 0x1e, 0x01, 0x11, 0x09, 0x19, 0x05, 0x15, 0x0d, 0x1d,
0x03, 0x13, 0x0b, 0x1b, 0x07, 0x17, 0x0f, 0x1f
];


pub(crate) struct InflaterManaged {
    output: /*readonly*/ OutputWindow,
    bits: /*readonly*/ BitsBuffer,
    literal_length_tree: Option<HuffmanTree>,
    distance_tree: Option<HuffmanTree>,

    state: InflaterState,
    bfinal: i32,
    block_type: BlockType,

    // uncompressed block
    block_length_buffer: [u8; 4],
    block_length: usize,

    // compressed block
    length: usize,
    distance_code: i32,
    extra_bits: i32,

    loop_counter: i32,
    literal_length_code_count: i32,
    distance_code_count: i32,
    code_length_code_count: i32,
    code_array_size: i32,
    length_code: i32,

    code_list: [u8; HuffmanTree::MAX_LITERAL_TREE_ELEMENTS + HuffmanTree::MAX_DIST_TREE_ELEMENTS],// temporary array to store the code length for literal/Length and distance
    code_length_tree_code_length: [u8; HuffmanTree::NUMBER_OF_CODE_LENGTH_TREE_ELEMENTS],
    deflate64: bool,
    code_length_tree: Option<HuffmanTree>,
    uncompressed_size: usize,
    current_inflated_count: usize,
}

impl InflaterManaged {
    fn new(deflate64: bool, uncompressed_size: usize) -> Self {
        Self {
            output: OutputWindow::new(),
            bits: BitsBuffer::new(),

            literal_length_tree: None,
            code_list: [0u8; HuffmanTree::MAX_LITERAL_TREE_ELEMENTS + HuffmanTree::MAX_DIST_TREE_ELEMENTS],
            code_length_tree_code_length: [0u8; HuffmanTree::NUMBER_OF_CODE_LENGTH_TREE_ELEMENTS],
            deflate64: deflate64,
            code_length_tree: None,
            uncompressed_size,
            state: InflaterState::ReadingBFinal, // start by reading BFinal bit
            bfinal: 0,
            block_type: BlockType::Uncompressed,
            block_length_buffer: [0u8; 4],
            block_length: 0,
            length: 0,
            distance_code: 0,
            extra_bits: 0,
            loop_counter: 0,
            literal_length_code_count: 0,
            distance_code_count: 0,
            code_length_code_count: 0,
            code_array_size: 0,
            distance_tree: None,
            length_code: 0,
            current_inflated_count: 0,
        }
    }

    pub fn finished(&self) -> bool {
        self.state == InflaterState::Done
    }

    #[allow(dead_code)]
    pub fn available_output(&self) -> usize {
        self.output.available_bytes()
    }

    pub fn inflate(&mut self, input_bytes: &[u8], mut bytes: &mut [u8]) -> InflateResult {
        // copy bytes from output to outputbytes if we have available bytes
        // if buffer is not filled up. keep decoding until no input are available
        // if decodeBlock returns false. Throw an exception.
        let mut result = InflateResult::new();
        let mut input = InputBuffer::new(self.bits, input_bytes);
        while
        {
            let mut copied = 0;
            if self.uncompressed_size == usize::MAX
            {
                copied = self.output.copy_to(bytes);
            } else {
                if self.uncompressed_size > self.current_inflated_count
                {
                    let len = min(bytes.len(), self.uncompressed_size - self.current_inflated_count);
                    bytes = &mut bytes[..len];
                    copied = self.output.copy_to(bytes);
                    self.current_inflated_count += copied;
                } else {
                    self.state = InflaterState::Done;
                    self.output.clear_bytes_used();
                }
            }
            if copied > 0
            {
                bytes = &mut bytes[copied..];
                result.bytes_written += copied;
            }

            if bytes.is_empty()
            {
                // filled in the bytes buffer
                //break;
                false
            } else {
                // decode will return false when more input is needed
                !self.finished() && self.decode(&mut input)
            }
        } {};

        self.bits = input.bits;
        result.bytes_consumed = input.read_bytes;
        return result;
    }

    fn decode(&mut self, input: &mut InputBuffer) -> bool {
        let mut eob = false;
        let result;

        if self.finished()
        {
            return true;
        }

        if self.state == InflaterState::ReadingBFinal
        {
            // reading bfinal bit
            // Need 1 bit
            if !input.ensure_bits_available(1) {
                return false;
            }

            self.bfinal = input.get_bits(1);
            self.state = InflaterState::ReadingBType;
        }

        if self.state == InflaterState::ReadingBType
        {
            // Need 2 bits
            if !input.ensure_bits_available(2)
            {
                self.state = InflaterState::ReadingBType;
                return false;
            }

            self.block_type = BlockType::from_int(input.get_bits(2)).expect("UnknownBlockType");
            match self.block_type {
                BlockType::Dynamic => {
                    self.state = InflaterState::ReadingNumLitCodes;
                }
                BlockType::Static => {
                    self.literal_length_tree = Some(HuffmanTree::static_literal_length_tree());
                    self.distance_tree = Some(HuffmanTree::static_distance_tree());
                    self.state = InflaterState::DecodeTop;
                }
                BlockType::Uncompressed => {
                    self.state = InflaterState::UncompressedAligning;
                }
            }
        }

        if self.block_type == BlockType::Dynamic
        {
            if self.state < InflaterState::DecodeTop
            {
                // we are reading the header
                result = self.decode_dynamic_block_header(input);
            }
            else
            {
                result = self.decode_block(input, &mut eob); // this can returns true when output is full
            }
        }
        else if self.block_type == BlockType::Static
        {
            result = self.decode_block(input, &mut eob);
        }
        else if self.block_type == BlockType::Uncompressed
        {
            result = self.decode_uncompressed_block(input, &mut eob);
        }
        else
        {
            panic!("UnknownBlockType");
        }

        //
        // If we reached the end of the block and the block we were decoding had
        // bfinal=1 (final block)
        //
        if eob && (self.bfinal != 0)
        {
            self.state = InflaterState::Done;
        }
        return result;
    }

    fn decode_uncompressed_block(&mut self, input: &mut InputBuffer, end_of_block: &mut bool) -> bool {
        *end_of_block = false;
        loop
        {
            match self.state {
                InflaterState::UncompressedAligning => {
                    input.skip_to_byte_boundary();
                    self.state = InflaterState::UncompressedByte1;
                    continue; //goto case InflaterState.UncompressedByte1; 
                }
                InflaterState::UncompressedByte1
                | InflaterState::UncompressedByte2
                | InflaterState::UncompressedByte3
                | InflaterState::UncompressedByte4
                => {
                    let bits = input.get_bits(8);
                    if bits < 0
                    {
                        return false;
                    }

                    self.block_length_buffer[(self.state - InflaterState::UncompressedByte1) as usize] = bits as u8;
                    if self.state == InflaterState::UncompressedByte4
                    {
                        self.block_length = self.block_length_buffer[0] as usize + (self.block_length_buffer[1] as usize) *256;
                        let block_length_complement: i32 = self.block_length_buffer[2] as i32 + (self.block_length_buffer[3] as i32) *256;

                        // make sure complement matches
                        if self.block_length as u16 != !block_length_complement as u16
                        {
                            panic!("InvalidBlockLength")
                        }
                    }

                    self.state = match self.state {
                        InflaterState::UncompressedByte1 => InflaterState::UncompressedByte2,
                        InflaterState::UncompressedByte2 => InflaterState::UncompressedByte3,
                        InflaterState::UncompressedByte3 => InflaterState::UncompressedByte4,
                        InflaterState::UncompressedByte4 => InflaterState::DecodingUncompressed,
                        _ => unreachable!(),
                    };
                }
                InflaterState::DecodingUncompressed => {
                    // Directly copy bytes from input to output.
                    let bytes_copied = self.output.copy_from(input, self.block_length);
                    self.block_length -= bytes_copied;

                    if self.block_length == 0
                    {
                        // Done with this block, need to re-init bit buffer for next block
                        self.state = InflaterState::ReadingBFinal;
                        *end_of_block = true;
                        return true;
                    }

                    // We can fail to copy all bytes for two reasons:
                    //    Running out of Input
                    //    running out of free space in output window
                    if self.output.free_bytes() == 0
                    {
                        return true;
                    }

                    return false;
                }
                _ => {
                    panic!("UnknownState");
                }
            }
        }
    }

    fn decode_block(&mut self, input: &mut InputBuffer, end_of_block_code_seen: &mut bool) -> bool {
        *end_of_block_code_seen = false;

        let mut free_bytes = self.output.free_bytes();   // it is a little bit faster than frequently accessing the property
        while free_bytes > 65536
        {
            // With Deflate64 we can have up to a 64kb length, so we ensure at least that much space is available
            // in the OutputWindow to avoid overwriting previous unflushed output data.

            let mut symbol;
            match self.state {
                InflaterState::DecodeTop => {
                    // decode an element from the literal tree

                    // TODO: optimize this!!!
                    symbol = self.literal_length_tree.as_mut().unwrap().get_next_symbol(input);
                    if symbol < 0
                    {
                        // running out of input
                        return false;
                    }

                    if symbol < 256
                    {
                        // literal
                        self.output.write(symbol as u8);
                        free_bytes -= 1;
                    }
                    else if symbol == 256
                    {
                        // end of block
                        *end_of_block_code_seen = true;
                        // Reset state
                        self.state = InflaterState::ReadingBFinal;
                        return true;
                    }
                    else
                    {
                        // length/distance pair
                        symbol -= 257;     // length code started at 257
                        if symbol < 8
                        {
                            symbol += 3;   // match length = 3,4,5,6,7,8,9,10
                            self.extra_bits = 0;
                        }
                        else if !self.deflate64 && symbol == 28
                        {
                            // extra bits for code 285 is 0
                            symbol = 258;             // code 285 means length 258
                            self.extra_bits = 0;
                        }
                        else
                        {
                            if symbol as usize >= EXTRA_LENGTH_BITS.len()
                            {
                                panic!("GenericInvalidData");
                            }
                            self.extra_bits = EXTRA_LENGTH_BITS[symbol as usize] as i32;
                            assert_ne!(self.extra_bits, 0, "We handle other cases separately!");
                        }
                        self.length = symbol.try_into().expect("GenericInvalidData");

                        self.state = InflaterState::HaveInitialLength;
                        continue//goto case InflaterState::HaveInitialLength;
                    }
                }
                InflaterState::HaveInitialLength => {
                    if self.extra_bits > 0
                    {
                        self.state = InflaterState::HaveInitialLength;
                        let bits = input.get_bits(self.extra_bits);
                        if bits < 0
                        {
                            return false;
                        }

                        if self.length >= LENGTH_BASE.len()
                        {
                            panic!("GenericInvalidData");
                            //throw new InvalidDataException(SR.GenericInvalidData);
                        }
                        self.length = LENGTH_BASE[self.length] as usize + bits as usize;
                    }
                    self.state = InflaterState::HaveFullLength;
                    continue// goto case InflaterState::HaveFullLength;
                }
                InflaterState::HaveFullLength => {
                    if self.block_type == BlockType::Dynamic
                    {
                        self.distance_code = self.distance_tree.as_mut().unwrap().get_next_symbol(input);
                    }
                    else
                    {
                        // get distance code directly for static block
                        self.distance_code = input.get_bits(5);
                        if self.distance_code >= 0
                        {
                            self.distance_code = STATIC_DISTANCE_TREE_TABLE[self.distance_code as usize] as i32;
                        }
                    }

                    if self.distance_code < 0
                    {
                        // running out input
                        return false;
                    }

                    self.state = InflaterState::HaveDistCode;
                    continue//goto case InflaterState.HaveDistCode;
                }

                InflaterState::HaveDistCode => {
                    // To avoid a table lookup we note that for distanceCode > 3,
                    // extra_bits = (distanceCode-2) >> 1
                    let offset: usize;
                    if self.distance_code > 3
                    {
                        self.extra_bits = (self.distance_code - 2) >> 1;
                        let bits = input.get_bits(self.extra_bits);
                        if bits < 0
                        {
                            return false;
                        }
                        offset = DISTANCE_BASE_POSITION[self.distance_code as usize] as usize + bits as usize;
                    }
                    else
                    {
                        offset = (self.distance_code + 1) as usize;
                    }

                    self.output.write_length_distance(self.length, offset);
                    free_bytes -= self.length;
                    self.state = InflaterState::DecodeTop;
                }

                _ => {
                    //Debug.Fail("check why we are here!");
                    panic!("UnknownState");
                }
            }
        }

        return true;
    }

    // Format of the dynamic block header:
    //      5 Bits: HLIT, # of Literal/Length codes - 257 (257 - 286)
    //      5 Bits: HDIST, # of Distance codes - 1        (1 - 32)
    //      4 Bits: HCLEN, # of Code Length codes - 4     (4 - 19)
    //
    //      (HCLEN + 4) x 3 bits: code lengths for the code length
    //          alphabet given just above, in the order: 16, 17, 18,
    //          0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15
    //
    //          These code lengths are interpreted as 3-bit integers
    //          (0-7); as above, a code length of 0 means the
    //          corresponding symbol (literal/length or distance code
    //          length) is not used.
    //
    //      HLIT + 257 code lengths for the literal/length alphabet,
    //          encoded using the code length Huffman code
    //
    //       HDIST + 1 code lengths for the distance alphabet,
    //          encoded using the code length Huffman code
    //
    // The code length repeat codes can cross from HLIT + 257 to the
    // HDIST + 1 code lengths.  In other words, all code lengths form
    // a single sequence of HLIT + HDIST + 258 values.
    fn decode_dynamic_block_header(&mut self, input: &mut InputBuffer) -> bool {
        'switch: loop {
            match self.state {
                InflaterState::ReadingNumLitCodes => {
                    self.literal_length_code_count = input.get_bits(5);
                    if self.literal_length_code_count < 0
                    {
                        return false;
                    }
                    self.literal_length_code_count += 257;
                    self.state = InflaterState::ReadingNumDistCodes;
                    continue 'switch //goto case InflaterState::ReadingNumDistCodes;
                }
                InflaterState::ReadingNumDistCodes => {
                    self.distance_code_count = input.get_bits(5);
                    if self.distance_code_count < 0
                    {
                        return false;
                    }
                    self.distance_code_count += 1;
                    self.state = InflaterState::ReadingNumCodeLengthCodes;
                    continue 'switch // goto case InflaterState::ReadingNumCodeLengthCodes;
                }
                InflaterState::ReadingNumCodeLengthCodes => {
                    self.code_length_code_count = input.get_bits(4);
                    if self.code_length_code_count < 0
                    {
                        return false;
                    }
                    self.code_length_code_count += 4;
                    self.loop_counter = 0;
                    self.state = InflaterState::ReadingCodeLengthCodes;
                    continue 'switch // goto case InflaterState::ReadingCodeLengthCodes;
                }
                InflaterState::ReadingCodeLengthCodes => {
                    while self.loop_counter < self.code_length_code_count
                    {
                        let bits = input.get_bits(3);
                        if bits < 0
                        {
                            return false;
                        }
                        self.code_length_tree_code_length[CODE_ORDER[self.loop_counter as usize] as usize] = bits as u8;
                        self.loop_counter += 1;
                    }

                    for i in self.code_length_code_count as usize..CODE_ORDER.len()
                    {
                        self.code_length_tree_code_length[CODE_ORDER[i] as usize] = 0;
                    }

                    // create huffman tree for code length
                    self.code_length_tree = Some(HuffmanTree::new(&self.code_length_tree_code_length));
                    self.code_array_size = self.literal_length_code_count + self.distance_code_count;
                    self.loop_counter = 0; // reset loop count

                    self.state = InflaterState::ReadingTreeCodesBefore;
                    continue 'switch // goto case InflaterState::ReadingTreeCodesBefore;
                }
                InflaterState::ReadingTreeCodesBefore | InflaterState::ReadingTreeCodesAfter => {
                    while self.loop_counter < self.code_array_size
                    {
                        if self.state == InflaterState::ReadingTreeCodesBefore
                        {
                            self.length_code = self.code_length_tree.as_mut().unwrap().get_next_symbol(input);
                            if self.length_code < 0
                            {
                                return false;
                            }
                        }

                        // The alphabet for code lengths is as follows:
                        //  0 - 15: Represent code lengths of 0 - 15
                        //  16: Copy the previous code length 3 - 6 times.
                        //  The next 2 bits indicate repeat length
                        //         (0 = 3, ... , 3 = 6)
                        //      Example:  Codes 8, 16 (+2 bits 11),
                        //                16 (+2 bits 10) will expand to
                        //                12 code lengths of 8 (1 + 6 + 5)
                        //  17: Repeat a code length of 0 for 3 - 10 times.
                        //    (3 bits of length)
                        //  18: Repeat a code length of 0 for 11 - 138 times
                        //    (7 bits of length)
                        if self.length_code <= 15
                        {
                            self.code_list[self.loop_counter as usize] = self.length_code as u8;
                            self.loop_counter += 1;
                        } else {
                            let repeat_count;
                            if self.length_code == 16
                            {
                                if !input.ensure_bits_available(2)
                                {
                                    self.state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                if self.loop_counter == 0
                                {
                                    // can't have "prev code" on first code
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                let previous_code = self.code_list[self.loop_counter as usize - 1];
                                repeat_count = input.get_bits(2) + 3;

                                if self.loop_counter + repeat_count > self.code_array_size
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeat_count {
                                    self.code_list[self.loop_counter as usize] = previous_code;
                                    self.loop_counter += 1;
                                }
                            } else if self.length_code == 17
                            {
                                if !input.ensure_bits_available(3)
                                {
                                    self.state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                repeat_count = input.get_bits(3) + 3;

                                if self.loop_counter + repeat_count > self.code_array_size
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeat_count {
                                    self.code_list[self.loop_counter as usize] = 0;
                                    self.loop_counter += 1;
                                }
                            } else {
                                // code == 18
                                if !input.ensure_bits_available(7)
                                {
                                    self.state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                repeat_count = input.get_bits(7) + 11;

                                if self.loop_counter + repeat_count > self.code_array_size
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeat_count {
                                    self.code_list[self.loop_counter as usize] = 0;
                                    self.loop_counter += 1;
                                }
                            }
                        }
                        self.state = InflaterState::ReadingTreeCodesBefore; // we want to read the next code.
                    }
                    break 'switch
                }
                _ => {
                    panic!("InvalidDataException: UnknownState");
                }
            }
        }

        let mut literal_tree_code_length = [0u8; HuffmanTree::MAX_LITERAL_TREE_ELEMENTS];
        let mut distance_tree_code_length = [0u8; HuffmanTree::MAX_DIST_TREE_ELEMENTS];

        // Create literal and distance tables
        array_copy(&self.code_list, &mut literal_tree_code_length, self.literal_length_code_count as usize);
        array_copy1(&self.code_list, self.literal_length_code_count as usize, &mut distance_tree_code_length, 0, self.distance_code_count as usize);

        // Make sure there is an end-of-block code, otherwise how could we ever end?
        if literal_tree_code_length[HuffmanTree::END_OF_BLOCK_CODE] == 0
        {
            //throw new InvalidDataException();
            panic!("InvalidDataException")
        }

        self.literal_length_tree = Some(HuffmanTree::new(&literal_tree_code_length));
        self.distance_tree = Some(HuffmanTree::new(&distance_tree_code_length));
        self.state = InflaterState::DecodeTop;
        return true;
    }
}

#[cfg(test)]
mod test {
    use crate::inflater_managed::InflaterManaged;

    static ZIP_FILE_DATA: &[u8] = include_bytes!("../test-assets/deflate64.zip");
    static BINARY_WAV_DATA: &[u8] = include_bytes!("../test-assets/folder/binary.wmv");
    const ZIP_FILE_ENTRY_HEADER_SIZE: usize = 30;

    #[test]
    fn binary_wav() {
        let offset: usize = 0;
        let file_name_len: usize = "binary.wav".len();
        let extra_field_len: usize = 0;
        let compressed_size: usize = 2669743;
        let uncompressed_size: usize = 2703788;

        let compressed_data = &ZIP_FILE_DATA[offset + ZIP_FILE_ENTRY_HEADER_SIZE + file_name_len + extra_field_len..][..compressed_size];
        let mut uncompressed_data = vec![0u8; uncompressed_size];
        assert_eq!(BINARY_WAV_DATA.len(), uncompressed_size);

        let mut inflater = Box::new(InflaterManaged::new(true, uncompressed_size));
        let output = inflater.inflate(compressed_data, &mut uncompressed_data);
        assert_eq!(output.bytes_consumed, compressed_size);
        assert_eq!(output.bytes_written, uncompressed_size);

        assert_eq!(uncompressed_data, BINARY_WAV_DATA);
    }
}
