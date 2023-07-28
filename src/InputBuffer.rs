use std::cmp::min;

pub(crate) struct InputBuffer<'a> {
    _buffer: &'a [u8],
    _bitBuffer: u32,
    _bitsInBuffer: i32,
}

impl <'a> InputBuffer<'a> {
    pub fn new() -> Self {
        Self {
            _buffer: &[],
            _bitBuffer: 0,
            _bitsInBuffer: 0,
        }
    }

    pub fn AvailableBits(&self) -> i32 {
        self._bitsInBuffer
    }

    pub fn AvailableBytes(&self) -> usize {
        self._buffer.len() + (self._bitsInBuffer / 4) as usize
    }

    pub fn EnsureBitsAvailable(&mut self, count: i32) -> bool {
        debug_assert!(0 < count && count <= 16, "count is invalid.");

        // manual inlining to improve perf
        if (self._bitsInBuffer < count)
        {
            if (self.NeedsInput())
            {
                return false;
            }

            // insert a byte to bitbuffer
            self._bitBuffer |= (self._buffer[0] as u32) << self._bitsInBuffer;
            self._buffer = &self._buffer[1..];
            self._bitsInBuffer += 8;

            if (self._bitsInBuffer < count)
            {
                if (self.NeedsInput())
                {
                    return false;
                }
                // insert a byte to bitbuffer
                self._bitBuffer |= (self._buffer[0] as u32) << self._bitsInBuffer;
                self._buffer = &self._buffer[1..];
                self._bitsInBuffer += 8;
            }
        }

        return true;
    }

    pub fn TryLoad16Bits(&mut self) -> u32 {
        if (self._bitsInBuffer < 8)
        {
            if (self._buffer.len() > 1)
            {
                self._bitBuffer |= (self._buffer[0] as u32) << self._bitsInBuffer;
                self._bitBuffer |= (self._buffer[1] as u32) << (self._bitsInBuffer + 8);
                self._buffer = &self._buffer[2..];
                self._bitsInBuffer += 16;
            } else if (self._buffer.len() != 0)
            {
                self._bitBuffer |= (self._buffer[0] as u32) << self._bitsInBuffer;
                self._buffer = &self._buffer[1..];
                self._bitsInBuffer += 8;
            }
        } else if (self._bitsInBuffer < 16)
        {
            if (!self._buffer.is_empty())
            {
                self._bitBuffer |= (self._buffer[0] as u32) << self._bitsInBuffer;
                self._buffer = &self._buffer[1..];
                self._bitsInBuffer += 8;
            }
        }

        return self._bitBuffer;
    }

    fn GetBitMask(&self, count: i32) -> u32 {
        (1 << count) - 1
    }

    pub fn GetBits(&mut self, count: i32) -> i32 {
        debug_assert!(0 < count && count <= 16, "count is invalid.");

        if (!self.EnsureBitsAvailable(count))
        {
            return -1;
        }

        let result = (self._bitBuffer & self.GetBitMask(count)) as i32;
        self._bitBuffer >>= count;
        self._bitsInBuffer -= count;
        return result;
    }

    pub fn CopyTo(&mut self, mut output: &mut [u8]) -> usize {
        debug_assert!(self._bitsInBuffer % 8 == 0);

        // Copy the bytes in bitBuffer first.
        let mut bytesFromBitBuffer = 0;
        while (self._bitsInBuffer > 0 && !output.is_empty())
        {
            output[0] = self._bitBuffer as u8;
            output = &mut output[1..];
            self._bitBuffer >>= 8;
            self._bitsInBuffer -= 8;
            bytesFromBitBuffer += 1;
        }

        if (output.is_empty())
        {
            return bytesFromBitBuffer;
        }

        let length = min(output.len(), self._buffer.len());
        output[..length].copy_from_slice(&self._buffer[..length]);
        self._buffer = &self._buffer[length..];
        return bytesFromBitBuffer + length;
    }
    // CopyTo

    pub fn NeedsInput(&self) -> bool {
        self._buffer.is_empty()
    }

    pub fn SetInput(&mut self, buffer: &'a [u8]) {
        if self._buffer.is_empty() {
            self._buffer = buffer;
        }
    }

    /// <summary>Skip n bits in the buffer.</summary>
    pub fn SkipBits(&mut self, n: i32)
    {
        debug_assert!(self._bitsInBuffer >= n, "No enough bits in the buffer, Did you call EnsureBitsAvailable?");
        self._bitBuffer >>= n;
        self._bitsInBuffer -= n;
    }

    /// <summary>Skips to the next byte boundary.</summary>
    pub fn SkipToByteBoundary(&mut self)
    {
        self._bitBuffer >>= (self._bitsInBuffer % 8);
        self._bitsInBuffer -= (self._bitsInBuffer % 8);
    }
}
