use crate::InputBuffer::InputBuffer;

pub(crate) struct HuffmanTree {
    _tableBits: i32,
    _table: Box<[i16]>,
    _left: Box<[i16]>,
    _right: Box<[i16]>,
    _codeLengthArray: Box<[u8]>,
    #[cfg(debug_assertions)]
    _codeArrayDebug: Box<[u32]>,
    _tableMask: i32,
}

impl HuffmanTree {
    pub(crate) const MaxLiteralTreeElements: usize = 288;
    pub(crate) const MaxDistTreeElements: usize = 32;
    pub(crate) const EndOfBlockCode: usize = 256;
    pub(crate) const NumberOfCodeLengthTreeElements: usize = 19;

    pub fn StaticLiteralLengthTree() -> Self {
        return HuffmanTree::new(&Self::GetStaticLiteralTreeLength());
    }

    pub fn StaticDistanceTree() -> Self {
        return HuffmanTree::new(&Self::GetStaticDistanceTreeLength());
    }

    pub fn new(codeLengths: &[u8]) -> HuffmanTree
    {
        debug_assert!(
            codeLengths.len() == Self::MaxLiteralTreeElements ||
                codeLengths.len() == Self::MaxDistTreeElements ||
                codeLengths.len() == Self::NumberOfCodeLengthTreeElements,
            "we only expect three kinds of Length here");
        let _codeLengthArray = codeLengths.to_vec().into_boxed_slice();

        let _tableBits = if (_codeLengthArray.len() == Self::MaxLiteralTreeElements)
        {
            // bits for Literal/Length tree table
            9
        } else {
            // bits for distance tree table and code length tree table
            7
        };
        let _tableMask = (1 << _tableBits) - 1;

        let _table = vec![0i16; 1 << _tableBits].into_boxed_slice();

        // I need to find proof that left and right array will always be
        // enough. I think they are.

        let _left = vec![0i16; 2 * _codeLengthArray.len()].into_boxed_slice();
        let _right = vec![0i16; 2 * _codeLengthArray.len()].into_boxed_slice();

        let mut instance = Self {
            _tableBits,
            _table,
            _left,
            _right,
            _codeLengthArray,
            _codeArrayDebug: Box::new([]),
            _tableMask,
        };

        instance.CreateTable();

        instance
    }

    // Generate the array contains huffman codes lengths for static huffman tree.
    // The data is in RFC 1951.
    fn GetStaticLiteralTreeLength() -> [u8; Self::MaxLiteralTreeElements] {
        let mut literalTreeLength = [0u8; Self::MaxLiteralTreeElements];

        literalTreeLength[0..][..144].fill(8);
        literalTreeLength[144..][..112].fill(9);
        literalTreeLength[256..][..24].fill(7);
        literalTreeLength[280..][..8].fill(8);
        return literalTreeLength;
    }

    fn GetStaticDistanceTreeLength() -> [u8; Self::MaxDistTreeElements] {
        return [5u8; Self::MaxDistTreeElements];
    }

    fn BitReverse(mut code: u32, mut length: usize) -> u32 {
        let mut new_code = 0;

        debug_assert!(length > 0 && length <= 16, "Invalid len");
        while {
            new_code |= (code & 1);
            new_code <<= 1;
            code >>= 1;

            length -= 1;
            length > 0
        } {}

        return new_code >> 1;
    }

    fn CalculateHuffmanCode(&self) -> [u32; Self::MaxLiteralTreeElements] {
        let mut bitLengthCount = [0u32; 17];
        for &codeLength in self._codeLengthArray.iter() {
            bitLengthCount[codeLength as usize] += 1;
        }
        bitLengthCount[0] = 0;  // clear count for length 0

        let mut nextCode = [0u32; 17];
        let mut tempCode = 0u32;

        for bits in 1..=16 {
            tempCode = (tempCode + bitLengthCount[bits - 1]) << 1;
            nextCode[bits] = tempCode;
        }

        let mut code = [0u32; Self::MaxLiteralTreeElements];
        for (i, &len) in self._codeLengthArray.iter().enumerate() {
            if (len > 0)
            {
                code[i] = Self::BitReverse(nextCode[len as usize], len as usize);
                nextCode[len as usize] += 1;
            }
        }

        return code;
    }

    fn CreateTable(&mut self) {
        let codeArray = self.CalculateHuffmanCode();
        #[cfg(debug_assertions)]
        {
            self._codeArrayDebug = Box::new(codeArray);
        }

        let mut avail = self._codeLengthArray.len() as i16;

        for (ch, &len) in self._codeLengthArray.iter().enumerate() {
            if (len > 0)
            {
                // start value (bit reversed)
                let mut start = codeArray[ch] as usize;

                if (len as i32 <= self._tableBits)
                {
                    // If a particular symbol is shorter than nine bits,
                    // then that symbol's translation is duplicated
                    // in all those entries that start with that symbol's bits.
                    // For example, if the symbol is four bits, then it's duplicated
                    // 32 times in a nine-bit table. If a symbol is nine bits long,
                    // it appears in the table once.
                    //
                    // Make sure that in the loop below, code is always
                    // less than table_size.
                    //
                    // On last iteration we store at array index:
                    //    initial_start_at + (locs-1)*increment
                    //  = initial_start_at + locs*increment - increment
                    //  = initial_start_at + (1 << tableBits) - increment
                    //  = initial_start_at + table_size - increment
                    //
                    // Therefore we must ensure:
                    //     initial_start_at + table_size - increment < table_size
                    // or: initial_start_at < increment
                    //
                    let increment = 1 << len;
                    if (start >= increment)
                    {
                        //throw new InvalidDataException(SR.InvalidHuffmanData);
                        panic!("InvalidHuffmanData");
                    }

                    // Note the bits in the table are reverted.
                    let locs = 1 << (self._tableBits - len as i32);
                    for j in 0..locs {
                        self._table[start] = ch as i16;
                        start += increment;
                    }
                }
                else
                {
                    // For any code which has length longer than num_elements,
                    // build a binary tree.

                    let mut overflowBits = len as i32 - self._tableBits; // the nodes we need to respent the data.
                    let mut codeBitMask = 1 << self._tableBits; // mask to get current bit (the bits can't fit in the table)

                    // the left, right table is used to repesent the
                    // the rest bits. When we got the first part (number bits.) and look at
                    // tbe table, we will need to follow the tree to find the real character.
                    // This is in place to avoid bloating the table if there are
                    // a few ones with long code.
                    let mut index = start & ((1 << self._tableBits) - 1);
                    let mut array = &mut self._table;

                    while
                    {
                        let mut value = array[index];

                        if (value == 0)
                        {
                            // set up next pointer if this node is not used before.
                            array[index] = -avail; // use next available slot.
                            value = -avail;
                            avail += 1;
                        }

                        if (value > 0)
                        {
                            // prevent an IndexOutOfRangeException from array[index]
                            panic!("InvalidHuffmanData");
                        }

                        debug_assert!(value < 0, "CreateTable: Only negative numbers are used for tree pointers!");

                        if ((start & codeBitMask) == 0)
                        {
                            // if current bit is 0, go change the left array
                            array = &mut self._left;
                        }
                        else {
                            // if current bit is 1, set value in the right array
                            array = &mut self._right;
                        }
                        index = -value as usize; // go to next node

                        codeBitMask <<= 1;
                        overflowBits -= 1;

                        overflowBits != 0
                    } {};

                    array[index] = ch as i16;
                }
            }
        }
    }

    pub fn GetNextSymbol(&mut self, input: &mut InputBuffer) -> i32 {

        // Try to load 16 bits into input buffer if possible and get the bitBuffer value.
        // If there aren't 16 bits available we will return all we have in the
        // input buffer.
        let bitBuffer = input.TryLoad16Bits();
        if (input.AvailableBits() == 0)
        {    // running out of input.
            return -1;
        }

        // decode an element
        let mut symbol = self._table[bitBuffer as usize & self._tableMask as usize] as i32;
        if (symbol < 0)
        {       //  this will be the start of the binary tree
            // navigate the tree
            let mut mask = 1 << self._tableBits;
            while
            {
                symbol = -symbol;
                if ((bitBuffer & mask) == 0) {
                    symbol = self._left[symbol as usize] as i32;
                }
                else {
                    symbol = self._right[symbol as usize] as i32;
                }
                mask <<= 1;
                (symbol < 0)
            } {};
        }

        let codeLength = self._codeLengthArray[symbol as usize] as i32;

        // huffman code lengths must be at least 1 bit long
        if (codeLength <= 0)
        {
            panic!("InvalidHuffmanData");
        }

        //
        // If this code is longer than the # bits we had in the bit buffer (i.e.
        // we read only part of the code), we can hit the entry in the table or the tree
        // for another symbol. However the length of another symbol will not match the
        // available bits count.
        if (codeLength > input.AvailableBits())
        {
            // We already tried to load 16 bits and maximum length is 15,
            // so this means we are running out of input.
            return -1;
        }

        input.SkipBits(codeLength);
        return symbol;
    }
}
