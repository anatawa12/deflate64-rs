use std::cmp::min;

#[derive(Copy, Clone)]
pub(crate) struct BitsBuffer {
    _bitBuffer: u32,
    _bitsInBuffer: i32,
}

impl BitsBuffer {
    pub(crate) fn new() -> BitsBuffer {
        Self {
            _bitBuffer: 0,
            _bitsInBuffer: 0,
        }
    }
}

pub(crate) struct InputBuffer<'a> {
    pub bits: BitsBuffer,
    pub buffer: &'a [u8],
    pub read_bytes: usize,
}

impl<'a> InputBuffer<'a> {
    pub fn new(bits: BitsBuffer, buffer: &'a [u8]) -> Self {
        Self { bits, buffer, read_bytes: 0 }
    }

    pub fn AvailableBits(&self) -> i32 {
        self.bits._bitsInBuffer
    }

    pub fn AvailableBytes(&self) -> usize {
        self.buffer.len() + (self.bits._bitsInBuffer / 4) as usize
    }

    pub fn EnsureBitsAvailable(&mut self, count: i32) -> bool {
        debug_assert!(0 < count && count <= 16, "count is invalid.");

        // manual inlining to improve perf
        if (self.bits._bitsInBuffer < count)
        {
            if (self.NeedsInput())
            {
                return false;
            }

            // insert a byte to bitbuffer
            self.bits._bitBuffer |= (self.buffer[0] as u32) << self.bits._bitsInBuffer;
            self.advance(1);
            self.bits._bitsInBuffer += 8;

            if (self.bits._bitsInBuffer < count)
            {
                if (self.NeedsInput())
                {
                    return false;
                }
                // insert a byte to bitbuffer
                self.bits._bitBuffer |= (self.buffer[0] as u32) << self.bits._bitsInBuffer;
                self.advance(1);
                self.bits._bitsInBuffer += 8;
            }
        }

        return true;
    }

    pub fn TryLoad16Bits(&mut self) -> u32 {
        if (self.bits._bitsInBuffer < 8)
        {
            if (self.buffer.len() > 1)
            {
                self.bits._bitBuffer |= (self.buffer[0] as u32) << self.bits._bitsInBuffer;
                self.bits._bitBuffer |= (self.buffer[1] as u32) << (self.bits._bitsInBuffer + 8);
                self.advance(2);
                self.bits._bitsInBuffer += 16;
            } else if (self.buffer.len() != 0)
            {
                self.bits._bitBuffer |= (self.buffer[0] as u32) << self.bits._bitsInBuffer;
                self.advance(1);
                self.bits._bitsInBuffer += 8;
            }
        } else if (self.bits._bitsInBuffer < 16)
        {
            if (!self.buffer.is_empty())
            {
                self.bits._bitBuffer |= (self.buffer[0] as u32) << self.bits._bitsInBuffer;
                self.advance(1);
                self.bits._bitsInBuffer += 8;
            }
        }

        return self.bits._bitBuffer;
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

        let result = (self.bits._bitBuffer & self.GetBitMask(count)) as i32;
        self.bits._bitBuffer >>= count;
        self.bits._bitsInBuffer -= count;
        return result;
    }

    pub fn CopyTo(&mut self, mut output: &mut [u8]) -> usize {
        debug_assert!(self.bits._bitsInBuffer % 8 == 0);

        // Copy the bytes in bitBuffer first.
        let mut bytesFromBitBuffer = 0;
        while (self.bits._bitsInBuffer > 0 && !output.is_empty())
        {
            output[0] = self.bits._bitBuffer as u8;
            output = &mut output[1..];
            self.bits._bitBuffer >>= 8;
            self.bits._bitsInBuffer -= 8;
            bytesFromBitBuffer += 1;
        }

        if (output.is_empty())
        {
            return bytesFromBitBuffer;
        }

        let length = min(output.len(), self.buffer.len());
        output[..length].copy_from_slice(&self.buffer[..length]);
        self.advance(length);
        return bytesFromBitBuffer + length;
    }
    // CopyTo

    pub fn NeedsInput(&self) -> bool {
        self.buffer.is_empty()
    }

    /// <summary>Skip n bits in the buffer.</summary>
    pub fn SkipBits(&mut self, n: i32)
    {
        debug_assert!(self.bits._bitsInBuffer >= n, "No enough bits in the buffer, Did you call EnsureBitsAvailable?");
        self.bits._bitBuffer >>= n;
        self.bits._bitsInBuffer -= n;
    }

    /// <summary>Skips to the next byte boundary.</summary>
    pub fn SkipToByteBoundary(&mut self)
    {
        self.bits._bitBuffer >>= (self.bits._bitsInBuffer % 8);
        self.bits._bitsInBuffer -= (self.bits._bitsInBuffer % 8);
    }

    fn advance(&mut self, buf: usize) {
        self.buffer = &self.buffer[buf..];
        self.read_bytes += buf;
    }
}
