# Checkpoints (experimental)

## Overview

`checkpoint()` and `restore_from_checkpoint()` allow partial decompression progress to be saved and restored, even across different processes. This can be used to restore partial progress after being interrupted when decompressing very long streams or when read/write storage access is very slow.

## Using Checkpoints for Recovery

```rust
pub fn checkpoint(&mut self) -> Option<(Vec<u8>, CheckpointStreamPositions)>

pub fn restore_from_checkpoint(&mut self, checkpoint_data: &[u8]) -> Option<CheckpointStreamPositions>

pub struct CheckpointStreamPositions {
    pub output_bytes_already_returned: u64,  // caller must skip this many output bytes
    pub input_bytes_to_skip: u64,            // caller must seek input to this byte offset
}
```

During decompression, the caller can periodically call `checkpoint()` in between `inflate()` invocations and record the checkpoint data to persistant storage, along with all previous inflation output bytes.

To resume, the caller should set up as if processing from the very beginning, and then:
1. Call `restore_from_checkpoint()`, which always succeeds unless the checkpoint data is corrupt  
2. Seek the input stream to `input_bytes_to_skip` as counted from the start of the stream
3. Seek the output stream to `output_bytes_already_returned` as counted from the start of the stream
4. Resume decompression with a traditional `inflate()` processing loop

Note, checkpoint data represents internal program state, and invalid or corrupt checkpoint data cannot always be detected. Do not restore checkpoints from untrusted sources as this may lead to decompression errors or incorrect inflate() output.

## Internal Details

When the "checkpoint" feature is enabled, the inflater keeps some additional internal variables:
- `checkpoint_input_bits`: exact input bit position at time of checkpoint
- `checkpoint_bit_buffer`: low byte of bit_buffer at time of checkpoint

These are updated after every internal write to the output window, when the decoder is in the DecodeTop or DecodingUncompressed internal states. To keep the implementation simple, checkpoints are not supported in between deflate blocks, or during block header parsing. As a result of that choice, restoring from a checkpoint can require a small amount of repeated input, but never produces any duplicated output.

| Location | Trigger |
|----------|---------|
| `decode_block()` after literal | After `output.write()` |
| `decode_block()` after LZ match | After `output.write_length_distance()` |
| `decode_uncompressed_block()` | After `output.copy_from()` |

The internal checkpoint values are combined with the current inflater state and serialized into a byte buffer when `checkpoint()` is called. The process is reversed by `restore_from_checkpoint()` which reconstructs internal state from the byte buffer, including the preserved contents of the output window.

## Serialization Format

The size of the byte buffer returned from `checkpoint()` will generally be 65KB, although it can be as large as 257KB if the inflater contains the maximum possible amount of buffered output which has not yet been drained by the caller.

The format is a fixed 346-byte header followed by variable-length window data and a trailing checksum:

```
Offset  Size  Field
------  ----  -----
0       8     input_bits: u64 LE              # exact input bit position
8       1     buffered_value: u8              # low byte of input bit buffer (contains 0-7 unread bits)
9       1     bfinal_block_type: u8           # (bfinal << 7) | block_type
10      4     uncompressed_remaining: u32 LE  # bytes left in uncompressed block (0 if not applicable)
14      288   lit_code_lengths: [u8; 288]     # dynamic block: code lengths, 0xFF-padded; else all 0xFF
302     32    dist_code_lengths: [u8; 32]     # dynamic block: code lengths, 0xFF-padded; else all 0xFF
334     8     output_bytes_written: u64 LE    # total bytes ever written to window
342     4     output_bytes_unread: u32 LE     # bytes in window not yet returned to caller
346     var   window_data: [u8]               # len = max(min(65538, output_bytes_written), output_bytes_unread)
END-4   4     checksum: u32 LE                # Fletcher-32 checksum of preceding bytes
```

The serialized window_data contains all "reachable" bytes from the output window. At a minimum, the includes the most recent 65538 bytes which can be referenced by DEFLATE64 distance codes. The output window also buffers output which has not yet been returned to the caller, and so if the caller is not draining output bytes fast enough, the checkpoint must include all unread bytes (up to 256KB, the window size).
