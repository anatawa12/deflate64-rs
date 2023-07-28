use std::cmp::min;
use crate::*;
use crate::HuffmanTree::HuffmanTree;
use crate::InputBuffer::InputBuffer;
use crate::OutputWindow::OutputWindow;

// Extra bits for length code 257 - 285.
static ExtraLengthBits: &'static [u8] = &[
0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3,
3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 16
];

// The base length for length code 257 - 285.
// The formula to get the real length for a length code is lengthBase[code - 257] + (value stored in extraBits)
static LengthBase: &'static [u8] = &[
3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51,
59, 67, 83, 99, 115, 131, 163, 195, 227, 3
];

// The base distance for distance code 0 - 31
// The real distance for a distance code is  distanceBasePosition[code] + (value stored in extraBits)
static DistanceBasePosition: &'static [u16] = &[
1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 32769, 49153
];

// code lengths for code length alphabet is stored in following order
static CodeOrder: &'static [u8] = &[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15 ];

static StaticDistanceTreeTable: &'static [u8] = &[
0x00, 0x10, 0x08, 0x18, 0x04, 0x14, 0x0c, 0x1c, 0x02, 0x12, 0x0a, 0x1a,
0x06, 0x16, 0x0e, 0x1e, 0x01, 0x11, 0x09, 0x19, 0x05, 0x15, 0x0d, 0x1d,
0x03, 0x13, 0x0b, 0x1b, 0x07, 0x17, 0x0f, 0x1f
];


pub(crate) struct InflaterManaged<'a> {
    _output: /*readonly*/ OutputWindow,
    _input: /*readonly*/ InputBuffer<'a>,
    _literalLengthTree: Option<HuffmanTree>,
    _distanceTree: Option<HuffmanTree>,

    _state: InflaterState,
    _bfinal: i32,
    _blockType: BlockType,

    // uncompressed block
    _blockLengthBuffer: [u8; 4],
    _blockLength: usize,

    // compressed block
    _length: usize,
    _distanceCode: i32,
    _extraBits: i32,

    _loopCounter: i32,
    _literalLengthCodeCount: i32,
    _distanceCodeCount: i32,
    _codeLengthCodeCount: i32,
    _codeArraySize: i32,
    _lengthCode: i32,

    _codeList: [u8; HuffmanTree::MaxLiteralTreeElements + HuffmanTree::MaxDistTreeElements],// temporary array to store the code length for literal/Length and distance
    _codeLengthTreeCodeLength: [u8; HuffmanTree::NumberOfCodeLengthTreeElements],
    _deflate64: bool,
    _codeLengthTree: Option<HuffmanTree>,
    /*readonly*/_uncompressedSize: usize,
    _currentInflatedCount: usize,
}

impl<'a> InflaterManaged<'a> {
    fn new(deflate64: bool, uncompressedSize: usize) -> Self {
        Self {
            _output: OutputWindow::new(),
            _input: InputBuffer::new(),

            _literalLengthTree: None,
            _codeList: [0u8; HuffmanTree::MaxLiteralTreeElements + HuffmanTree::MaxDistTreeElements],
            _codeLengthTreeCodeLength: [0u8; HuffmanTree::NumberOfCodeLengthTreeElements],
            _deflate64: deflate64,
            _codeLengthTree: None,
            _uncompressedSize: uncompressedSize,
            _state: InflaterState::ReadingBFinal, // start by reading BFinal bit
            _bfinal: 0,
            _blockType: BlockType::Uncompressed,
            _blockLengthBuffer: [0u8; 4],
            _blockLength: 0,
            _length: 0,
            _distanceCode: 0,
            _extraBits: 0,
            _loopCounter: 0,
            _literalLengthCodeCount: 0,
            _distanceCodeCount: 0,
            _codeLengthCodeCount: 0,
            _codeArraySize: 0,
            _distanceTree: None,
            _lengthCode: 0,
            _currentInflatedCount: 0,
        }
    }

    pub fn SetInput(&mut self, inputBytes: &'a [u8]) {
        self._input.SetInput(inputBytes);
    }

    pub fn Finished(&self) -> bool {
        self._state == InflaterState::Done || self._state == InflaterState::VerifyingFooter
    }

    pub fn AvailableOutput(&self) -> usize {
        self._output.AvailableBytes()
    }

    pub fn Inflate(&mut self, mut bytes: &mut [u8]) -> usize {
        // copy bytes from output to outputbytes if we have available bytes
        // if buffer is not filled up. keep decoding until no input are available
        // if decodeBlock returns false. Throw an exception.
        let mut count = 0;
        while
        {
            let mut copied = 0;
            if (self._uncompressedSize == usize::MAX)
            {
                copied = self._output.CopyTo(bytes);
            } else {
                if (self._uncompressedSize > self._currentInflatedCount)
                {
                    let len = min(bytes.len(), (self._uncompressedSize - self._currentInflatedCount));
                    bytes = &mut bytes[..len];
                    copied = self._output.CopyTo(bytes);
                    self._currentInflatedCount += copied;
                } else {
                    self._state = InflaterState::Done;
                    self._output.ClearBytesUsed();
                }
            }
            if (copied > 0)
            {
                bytes = &mut bytes[copied..];
                count += copied;
            }

            if (bytes.is_empty())
            {
                // filled in the bytes buffer
                //break;
                return count;
            }
            // Decode will return false when more input is needed
            !self.Finished() && self.Decode()
        } {};

        return count;
    }

    fn Decode(&mut self) -> bool {
        let mut eob = false;
        let mut result;

        if (self.Finished())
        {
            return true;
        }

        if (self._state == InflaterState::ReadingBFinal)
        {
            // reading bfinal bit
            // Need 1 bit
            if (!self._input.EnsureBitsAvailable(1)) {
                return false;
            }

            self._bfinal = self._input.GetBits(1);
            self._state = InflaterState::ReadingBType;
        }

        if (self._state == InflaterState::ReadingBType)
        {
            // Need 2 bits
            if (!self._input.EnsureBitsAvailable(2))
            {
                self._state = InflaterState::ReadingBType;
                return false;
            }

            self._blockType = BlockType::from_int(self._input.GetBits(2)).expect("UnknownBlockType");
            match self._blockType {
                BlockType::Dynamic => {
                    self._state = InflaterState::ReadingNumLitCodes;
                }
                BlockType::Static => {
                    self._literalLengthTree = Some(HuffmanTree::StaticLiteralLengthTree());
                    self._distanceTree = Some(HuffmanTree::StaticDistanceTree());
                    self._state = InflaterState::DecodeTop;
                }
                BlockType::Uncompressed => {
                    self._state = InflaterState::UncompressedAligning;
                }
            }
        }

        if self._blockType == BlockType::Dynamic
        {
            if self._state < InflaterState::DecodeTop
            {
                // we are reading the header
                result = self.DecodeDynamicBlockHeader();
            }
            else
            {
                result = self.DecodeBlock(&mut eob); // this can returns true when output is full
            }
        }
        else if (self._blockType == BlockType::Static)
        {
            result = self.DecodeBlock(&mut eob);
        }
        else if (self._blockType == BlockType::Uncompressed)
        {
            result = self.DecodeUncompressedBlock(&mut eob);
        }
        else
        {
            panic!("UnknownBlockType");
        }

        //
        // If we reached the end of the block and the block we were decoding had
        // bfinal=1 (final block)
        //
        if (eob && (self._bfinal != 0))
        {
            self._state = InflaterState::Done;
        }
        return result;
    }

    fn DecodeUncompressedBlock(&mut self, end_of_block: &mut bool) -> bool {
        *end_of_block = false;
        loop
        {
            match self._state {
                InflaterState::UncompressedAligning => {
                    self._input.SkipToByteBoundary();
                    self._state = InflaterState::UncompressedByte1;
                    continue; //goto case InflaterState.UncompressedByte1; 
                }
                InflaterState::UncompressedByte1
                | InflaterState::UncompressedByte2
                | InflaterState::UncompressedByte3
                | InflaterState::UncompressedByte4
                => {
                    let bits = self._input.GetBits(8);
                    if (bits < 0)
                    {
                        return false;
                    }

                    self._blockLengthBuffer[(self._state - InflaterState::UncompressedByte1) as usize] = bits as u8;
                    if (self._state == InflaterState::UncompressedByte4)
                    {
                        self._blockLength = self._blockLengthBuffer[0] as usize + (self._blockLengthBuffer[1] as usize) *256;
                        let blockLengthComplement: i32 = self._blockLengthBuffer[2] as i32 + (self._blockLengthBuffer[3] as i32) *256;

                        // make sure complement matches
                        if (self._blockLength as u16 != !blockLengthComplement as u16)
                        {
                            panic!("InvalidBlockLength")
                        }
                    }

                    self._state = match self._state {
                        InflaterState::UncompressedByte1 => InflaterState::UncompressedByte2,
                        InflaterState::UncompressedByte2 => InflaterState::UncompressedByte3,
                        InflaterState::UncompressedByte3 => InflaterState::UncompressedByte4,
                        InflaterState::UncompressedByte4 => InflaterState::DecodingUncompressed,
                        _ => unreachable!(),
                    };
                }
                InflaterState::DecodingUncompressed => {
                    // Directly copy bytes from input to output.
                    let bytesCopied = self._output.CopyFrom(&mut self._input, self._blockLength);
                    self._blockLength -= bytesCopied;

                    if (self._blockLength == 0)
                    {
                        // Done with this block, need to re-init bit buffer for next block
                        self._state = InflaterState::ReadingBFinal;
                        *end_of_block = true;
                        return true;
                    }

                    // We can fail to copy all bytes for two reasons:
                    //    Running out of Input
                    //    running out of free space in output window
                    if (self._output.FreeBytes() == 0)
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

    fn DecodeBlock(&mut self, end_of_block_code_seen: &mut bool) -> bool {
        *end_of_block_code_seen = false;

        let mut freeBytes = self._output.FreeBytes();   // it is a little bit faster than frequently accessing the property
        while (freeBytes > 65536)
        {
            // With Deflate64 we can have up to a 64kb length, so we ensure at least that much space is available
            // in the OutputWindow to avoid overwriting previous unflushed output data.

            let mut symbol;
            match self._state {
                InflaterState::DecodeTop => {
                    // decode an element from the literal tree

                    // TODO: optimize this!!!
                    symbol = self._literalLengthTree.as_mut().unwrap().GetNextSymbol(&mut self._input);
                    if (symbol < 0)
                    {
                        // running out of input
                        return false;
                    }

                    if (symbol < 256)
                    {
                        // literal
                        self._output.Write(symbol as u8);
                        freeBytes -= 1;
                    }
                    else if (symbol == 256)
                    {
                        // end of block
                        *end_of_block_code_seen = true;
                        // Reset state
                        self._state = InflaterState::ReadingBFinal;
                        return true;
                    }
                    else
                    {
                        // length/distance pair
                        symbol -= 257;     // length code started at 257
                        if (symbol < 8)
                        {
                            symbol += 3;   // match length = 3,4,5,6,7,8,9,10
                            self._extraBits = 0;
                        }
                        else if (!self._deflate64 && symbol == 28)
                        {
                            // extra bits for code 285 is 0
                            symbol = 258;             // code 285 means length 258
                            self._extraBits = 0;
                        }
                        else
                        {
                            if (symbol as usize >= ExtraLengthBits.len())
                            {
                                panic!("GenericInvalidData");
                            }
                            self._extraBits = ExtraLengthBits[symbol as usize] as i32;
                            assert!(self._extraBits != 0, "We handle other cases separately!");
                        }
                        self._length = symbol as usize;

                        self._state = InflaterState::HaveInitialLength;
                        continue//goto case InflaterState::HaveInitialLength;
                    }
                }
                InflaterState::HaveInitialLength => {
                    if (self._extraBits > 0)
                    {
                        self._state = InflaterState::HaveInitialLength;
                        let bits = self._input.GetBits(self._extraBits);
                        if (bits < 0)
                        {
                            return false;
                        }

                        if (self._length < 0 || self._length >= LengthBase.len())
                        {
                            panic!("GenericInvalidData");
                            //throw new InvalidDataException(SR.GenericInvalidData);
                        }
                        self._length = (LengthBase[self._length] as usize + bits as usize);
                    }
                    self._state = InflaterState::HaveFullLength;
                    continue// goto case InflaterState::HaveFullLength;
                }
                InflaterState::HaveFullLength => {
                    if (self._blockType == BlockType::Dynamic)
                    {
                        self._distanceCode = self._distanceTree.as_mut().unwrap().GetNextSymbol(&mut self._input);
                    }
                    else
                    {
                        // get distance code directly for static block
                        self._distanceCode = self._input.GetBits(5) as i32;
                        if (self._distanceCode >= 0)
                        {
                            self._distanceCode = StaticDistanceTreeTable[self._distanceCode as usize] as i32;
                        }
                    }

                    if (self._distanceCode < 0)
                    {
                        // running out input
                        return false;
                    }

                    self._state = InflaterState::HaveDistCode;
                    continue//goto case InflaterState.HaveDistCode;
                }

                InflaterState::HaveDistCode => {
                    // To avoid a table lookup we note that for distanceCode > 3,
                    // extra_bits = (distanceCode-2) >> 1
                    let mut offset: usize;
                    if (self._distanceCode > 3)
                    {
                        self._extraBits = (self._distanceCode - 2) >> 1;
                        let bits = self._input.GetBits(self._extraBits);
                        if (bits < 0)
                        {
                            return false;
                        }
                        offset = (DistanceBasePosition[self._distanceCode as usize] as usize + bits as usize);
                    }
                    else
                    {
                        offset = (self._distanceCode + 1) as usize;
                    }

                    self._output.WriteLengthDistance(self._length, offset);
                    freeBytes -= self._length;
                    self._state = InflaterState::DecodeTop;
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
    fn DecodeDynamicBlockHeader(&mut self) -> bool {
        'switch: loop {
            match self._state {
                InflaterState::ReadingNumLitCodes => {
                    self._literalLengthCodeCount = self._input.GetBits(5);
                    if (self._literalLengthCodeCount < 0)
                    {
                        return false;
                    }
                    self._literalLengthCodeCount += 257;
                    self._state = InflaterState::ReadingNumDistCodes;
                    continue 'switch //goto case InflaterState::ReadingNumDistCodes;
                }
                InflaterState::ReadingNumDistCodes => {
                    self._distanceCodeCount = self._input.GetBits(5);
                    if (self._distanceCodeCount < 0)
                    {
                        return false;
                    }
                    self._distanceCodeCount += 1;
                    self._state = InflaterState::ReadingNumCodeLengthCodes;
                    continue 'switch // goto case InflaterState::ReadingNumCodeLengthCodes;
                }
                InflaterState::ReadingNumCodeLengthCodes => {
                    self._codeLengthCodeCount = self._input.GetBits(4);
                    if (self._codeLengthCodeCount < 0)
                    {
                        return false;
                    }
                    self._codeLengthCodeCount += 4;
                    self._loopCounter = 0;
                    self._state = InflaterState::ReadingCodeLengthCodes;
                    continue 'switch // goto case InflaterState::ReadingCodeLengthCodes;
                }
                InflaterState::ReadingCodeLengthCodes => {
                    while (self._loopCounter < self._codeLengthCodeCount)
                    {
                        let bits = self._input.GetBits(3);
                        if (bits < 0)
                        {
                            return false;
                        }
                        self._codeLengthTreeCodeLength[CodeOrder[self._loopCounter as usize] as usize] = bits as u8;
                        self._loopCounter += 1;
                    }

                    for i in self._codeLengthCodeCount as usize..CodeOrder.len()
                    {
                        self._codeLengthTreeCodeLength[CodeOrder[i] as usize] = 0;
                    }

                    // create huffman tree for code length
                    self._codeLengthTree = Some(HuffmanTree::new(&self._codeLengthTreeCodeLength));
                    self._codeArraySize = self._literalLengthCodeCount + self._distanceCodeCount;
                    self._loopCounter = 0; // reset loop count

                    self._state = InflaterState::ReadingTreeCodesBefore;
                    continue 'switch // goto case InflaterState::ReadingTreeCodesBefore;
                }
                InflaterState::ReadingTreeCodesBefore | InflaterState::ReadingTreeCodesAfter => {
                    while (self._loopCounter < self._codeArraySize)
                    {
                        if (self._state == InflaterState::ReadingTreeCodesBefore)
                        {
                            self._lengthCode = self._codeLengthTree.as_mut().unwrap().GetNextSymbol(&mut self._input);
                            if (self._lengthCode < 0)
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
                        if (self._lengthCode <= 15)
                        {
                            self._codeList[self._loopCounter as usize] = self._lengthCode as u8;
                            self._loopCounter += 1;
                        } else {
                            let mut repeatCount;
                            if (self._lengthCode == 16)
                            {
                                if (!self._input.EnsureBitsAvailable(2))
                                {
                                    self._state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                if (self._loopCounter == 0)
                                {
                                    // can't have "prev code" on first code
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                let previousCode = self._codeList[self._loopCounter as usize - 1];
                                repeatCount = self._input.GetBits(2) + 3;

                                if (self._loopCounter + repeatCount > self._codeArraySize)
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeatCount {
                                    self._codeList[self._loopCounter as usize] = previousCode;
                                    self._loopCounter += 1;
                                }
                            } else if (self._lengthCode == 17)
                            {
                                if (!self._input.EnsureBitsAvailable(3))
                                {
                                    self._state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                repeatCount = self._input.GetBits(3) + 3;

                                if (self._loopCounter + repeatCount > self._codeArraySize)
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeatCount {
                                    self._codeList[self._loopCounter as usize] = 0;
                                    self._loopCounter += 1;
                                }
                            } else {
                                // code == 18
                                if (!self._input.EnsureBitsAvailable(7))
                                {
                                    self._state = InflaterState::ReadingTreeCodesAfter;
                                    return false;
                                }

                                repeatCount = self._input.GetBits(7) + 11;

                                if (self._loopCounter + repeatCount > self._codeArraySize)
                                {
                                    //throw new InvalidDataException();
                                    panic!()
                                }

                                for _ in 0..repeatCount {
                                    self._codeList[self._loopCounter as usize] = 0;
                                    self._loopCounter += 1;
                                }
                            }
                        }
                        self._state = InflaterState::ReadingTreeCodesBefore; // we want to read the next code.
                    }
                    break 'switch
                }
                _ => {
                    panic!("InvalidDataException: UnknownState");
                }
            }
        }

        let mut literalTreeCodeLength = [0u8; HuffmanTree::MaxLiteralTreeElements];
        let mut distanceTreeCodeLength = [0u8; HuffmanTree::MaxDistTreeElements];

        // Create literal and distance tables
        Array.Copy(&self._codeList, &mut literalTreeCodeLength, self._literalLengthCodeCount as usize);
        Array.Copy1(&self._codeList, self._literalLengthCodeCount as usize, &mut distanceTreeCodeLength, 0, self._distanceCodeCount as usize);

        // Make sure there is an end-of-block code, otherwise how could we ever end?
        if (literalTreeCodeLength[HuffmanTree::EndOfBlockCode] == 0)
        {
            //throw new InvalidDataException();
            panic!("InvalidDataException")
        }

        self._literalLengthTree = Some(HuffmanTree::new(&literalTreeCodeLength));
        self._distanceTree = Some(HuffmanTree::new(&distanceTreeCodeLength));
        self._state = InflaterState::DecodeTop;
        return true;
    }
}
