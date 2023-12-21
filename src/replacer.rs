/*!
 * This module enables AhoCorasickReplacer, which is used to manually work with chunks of data
*/
use alloc::{sync::Arc, vec::Vec, collections::VecDeque};

use crate::{automaton::{StateID, Automaton}, MatchError, Anchored, ahocorasick::AcAutomaton, AhoCorasickKind};

/// The replacer iself
pub struct AhoCorasickReplacer {
    aut: Arc<dyn AcAutomaton>,
    kind: AhoCorasickKind,
    sid: StateID,
    replace_with: Vec<Vec<u8>>,
    buffer: Vec<u8>, // Buffer holding the replaced data
    potential_buffer: VecDeque<u8>, // Buffer holding the start of a potential match
}

impl AhoCorasickReplacer
{
    /// Instantiate a new Replacer
    pub(crate) fn new(
        aut: Arc<dyn AcAutomaton>,
        kind: AhoCorasickKind,
        replace_with: Vec<Vec<u8>>,
    ) -> Result<Self, MatchError> {
        let sid = aut.start_state(Anchored::No)?;
        Ok(Self {
            aut,
            kind,
            sid,
            replace_with,
            buffer: Vec::new(),
            potential_buffer: VecDeque::new(),
        })
    }

    /// Writing to the buffer while making rare incremental resizes
    #[inline(always)]
    fn write_to_buffer(buf: &mut Vec<u8>, idx: &mut usize, char: u8) {
        if *idx >= buf.len() {
            // Since this function is called with incremental idx, we simply double current buffer length every time
            buf.resize(buf.len() * 2, b'\0');
        }
        buf[*idx] = char;
        *idx += 1;
    }

    /// Perform potential replacements in the chunk, reading the reference to the internal buffer containing the chunk data with eventually replaced bytes.
    /// self reference might be of 0 length even if the input was non-zero,
    /// because it might be holding onto a potential match without being able to decide whether replace or discard it yet
    pub fn replace(&mut self, chunk: &[u8]) -> Result<&[u8], MatchError> {
        let aut = self.aut.as_ref().coerce_concrete(self.kind);
        if self.buffer.len() < chunk.len() + self.potential_buffer.len() {
            // Default buffer length to chunk once to avoid incremental size increases & capacity reallocations during the buffer writing process
            self.buffer.resize(chunk.len() + self.potential_buffer.len(), b'\0');
        }
        let mut write_idx = 0usize;
        for byte in chunk {
            self.sid = aut.next_state(Anchored::No, self.sid, *byte);
            if aut.is_start(self.sid) {
                // No potential replacements
                while self.potential_buffer.len() > 0 {
                    // At self point potential buffer is discareded (written)
                    Self::write_to_buffer(
                        &mut self.buffer,
                        &mut write_idx,
                        self.potential_buffer.pop_front().unwrap(),
                    );
                }
                Self::write_to_buffer(&mut self.buffer, &mut write_idx, *byte);
            } else {
                self.potential_buffer.push_back(*byte);
                if aut.is_match(self.sid) {
                    let pattern_id = aut.match_pattern(self.sid, 0);
                    let pattern_len = aut.pattern_len(pattern_id);
                    // Either we followed a potential word all the way down, or we jumped to a different branch following the suffix link
                    // In the second case, we need to discard (write away) first part of the potential buffer, as it will be bigger than the max match,
                    // keeping as new potential the last part containing the amount of bytes equal to the new state node depth (equal to the pattern_len)
                    while self.potential_buffer.len() > pattern_len {
                        Self::write_to_buffer(
                            &mut self.buffer,
                            &mut write_idx,
                            self.potential_buffer.pop_front().unwrap(),
                        );
                    }

                    let replacement: &Vec<u8> = self.replace_with[pattern_id].as_ref();
                    // Replacement is given by the automaton node, so we only need to clear the potential buffer
                    self.potential_buffer.clear();
                    for replaced_byte in replacement.iter() {
                        Self::write_to_buffer(
                            &mut self.buffer,
                            &mut write_idx,
                            *replaced_byte,
                        );
                    }
                    // Reset the state after a replacement
                    self.sid =
                        aut.start_state(Anchored::No)?;
                }
            }
        }
        // Now (unless chunk was empty), either the bytes are in the buffer ready to be written, or they are in the potential buffer awaiting for the next chunk before being written
        // In both cases, all of them are considered "written" from the standpoint of AhoCorasickAsyncWriter, and we need to return not how many we have actually written to the sink with replacements,
        // but how many we have "consumed" - which should always match the length of input chunk. So the resulting byte count is independent from write_idx
        if write_idx > 0 {
            return Ok(&self.buffer[..write_idx]);
        } else if self.potential_buffer.len() > 0 {
            // Nothing written, but potential buffer is not empty - request immediate poll again with new buffer by saying we have accepted the buffer fully
            // This case happens when the potential buffer (replacement word length) exceeds the current chunk size while matching the entire chunk :
            // nothing can be written yet, but next chunk(s) are needed to determine the outcome (discard as-is, or replace)
            return Ok(&self.buffer[..0])
        } else {
            // This case can happen in 2 scenarios :
            // 1. Input chunk is empty (most likely a bug on the consumer side)
            // 2. The contents of chunk match entirely a word which has the empty string replacement
            return Ok(&self.buffer[..0])
        }
    }

    /// Returns the potentially buffered bytes of the last chunk
    pub fn finish(&mut self) -> Result<&[u8], MatchError> {
        if self.potential_buffer.len() > 0 {
            self.potential_buffer.make_contiguous();
            Ok(self.potential_buffer.as_slices().0)
        } else {
            Ok(&self.buffer[..0])
        }
    }
}