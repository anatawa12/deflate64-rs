use std::cmp::min;
use crate::InputBuffer::InputBuffer;

// With Deflate64 we can have up to a 65536 length as well as up to a 65538 distance. This means we need a Window that is at
// least 131074 bytes long so we have space to retrieve up to a full 64kb in lookback and place it in our buffer without
// overwriting existing data. OutputWindow requires that the WindowSize be an exponent of 2, so we round up to 2^18.
const WindowSize: usize = 262144;
const WindowMask: usize = 262143;

/// <summary>
/// This class maintains a window for decompressed output.
/// We need to keep this because the decompressed information can be
/// a literal or a length/distance pair. For length/distance pair,
/// we need to look back in the output window and copy bytes from there.
/// We use a byte array of WindowSize circularly.
/// </summary>
pub(crate) struct OutputWindow {
    _window: [u8; WindowSize],
    _end: usize,
    _bytesUsed: usize,
}

impl OutputWindow {
    pub fn new() -> Self {
        Self {
            _window: [0; WindowSize],
            _end: 0,
            _bytesUsed: 0,
        }
    }

    pub(crate) fn ClearBytesUsed(&mut self) {
        self._bytesUsed = 0;
    }

    /// <summary>Add a byte to output window.</summary>
    pub fn Write(&mut self, b: u8) {
        debug_assert!(self._bytesUsed < WindowSize, "Can't add byte when window is full!");
        self._window[self._end] = b;
        self._end += 1;
        self._end &= WindowMask;
        self._bytesUsed += 1;
        ;
    }

    pub fn WriteLengthDistance(&mut self, mut length: usize, distance: usize)
    {
        debug_assert!((self._bytesUsed + length) <= WindowSize, "No Enough space");

        // move backwards distance bytes in the output stream,
        // and copy length bytes from this position to the output stream.
        self._bytesUsed += length;
        let mut copyStart = (self._end.overflowing_sub(distance).0) & WindowMask; // start position for coping.

        let border = WindowSize - length;
        if (copyStart <= border && self._end < border)
        {
            if (length <= distance)
            {
                // src, srcIdx, dst, dstIdx, len
                // Array.Copy(self._window, copyStart, self._window, self._end, length);
                unsafe {
                    // src, dst, cnt
                    std::ptr::copy(
                        self._window.as_ptr().add(copyStart),
                        self._window.as_mut_ptr().add(self._end),
                        length
                    )
                }
                self._end += length;
            } else {
                // The referenced string may overlap the current
                // position; for example, if the last 2 bytes decoded have values
                // X and Y, a string reference with <length = 5, distance = 2>
                // adds X,Y,X,Y,X to the output stream.
                while (length > 0)
                {
                    length -= 1;
                    self._window[self._end] = self._window[copyStart];
                    self._end += 1;
                    copyStart += 1;
                }
            }
        } else {
            // copy byte by byte
            while (length > 0)
            {
                length -= 1;
                self._window[self._end] = self._window[copyStart];
                self._end += 1;
                copyStart += 1;
                self._end &= WindowMask;
                copyStart &= WindowMask;
            }
        }
    }

    /// <summary>
    /// Copy up to length of bytes from input directly.
    /// This is used for uncompressed block.
    /// </summary>
    pub fn CopyFrom(&mut self, input: &mut InputBuffer, mut length: usize) -> usize
    {
        length = min(min(length, WindowSize - self._bytesUsed), input.AvailableBytes());
        let mut copied: usize;

        // We might need wrap around to copy all bytes.
        let tailLen = WindowSize - self._end;
        if (length > tailLen)
        {
            // copy the first part
            copied = input.CopyTo(&mut self._window[self._end..][..tailLen]);
            if (copied == tailLen)
            {
                // only try to copy the second part if we have enough bytes in input
                copied += input.CopyTo(&mut self._window[..length - tailLen]);
            }
        } else {
            // only one copy is needed if there is no wrap around.
            copied = input.CopyTo(&mut self._window[self._end..][..tailLen]);
        }

        self._end = (self._end + copied) & WindowMask;
        self._bytesUsed += copied;
        return copied;
    }

    /// <summary>Free space in output window.</summary>
    pub fn FreeBytes(&self) -> usize {
        WindowSize - self._bytesUsed
    }

    /// <summary>Bytes not consumed in output window.</summary>
    pub fn AvailableBytes(&self) -> usize {
        self._bytesUsed
    }

    /// <summary>Copy the decompressed bytes to output buffer.</summary>
    pub fn CopyTo(&mut self, mut output: &mut [u8]) -> usize
    {
        let copy_end;

        if (output.len() > self._bytesUsed)
        {
            // we can copy all the decompressed bytes out
            copy_end = self._end;
            output = &mut output[..self._bytesUsed];
        } else {
            copy_end = (self._end - self._bytesUsed + output.len()) & WindowMask; // copy length of bytes
        }

        let copied = output.len();

        if (output.len() > copy_end)
        {
            let tailLen = output.len() - copy_end;
            // this means we need to copy two parts separately
            // copy the taillen bytes from the end of the output window
            output[..tailLen].copy_from_slice(&self._window[WindowSize - tailLen..][..tailLen]);
            output = &mut output[tailLen..][..copy_end];
        }
        output.copy_from_slice(&self._window[copy_end - output.len()..][..output.len()]);
        self._bytesUsed -= copied;
        debug_assert!(self._bytesUsed >= 0, "check this function and find why we copied more bytes than we have");
        return copied;
    }
}
