use crate::resp::RespValue;
use bytes::{Buf, BytesMut}; // Add Buf trait
use memchr::memchr;
use std::borrow::Cow;
use tracing::debug;

const MAX_ITERATIONS: usize = 1024;
const CRLF_LEN: usize = 2;
const DEFAULT_BUFFER_INIT_SIZE: usize = 4096;

type ParseResult = Result<Option<RespValue<'static>>, ParseError>;

#[derive(Debug, PartialEq, Clone)]
pub enum ParseError {
    InvalidFormat(Cow<'static, str>),
    InvalidLength,
    UnexpectedEof,
    Overflow,
    NotEnoughData,
    InvalidDepth,
    InvalidUtf8,
}

#[derive(Debug, PartialEq, Clone)]
#[repr(C, align(8))]
pub enum ParseState {
    Index {
        pos: usize,
    },
    ReadingLength {
        pos: usize,
        value: i64,
        negative: bool,
        type_char: u8,
    },
    ReadingBulkString {
        start_pos: usize,
        remaining: usize,
    },
    ReadingSimpleString {
        pos: usize,
    },
    ReadingError {
        pos: usize,
    },
    ReadingInteger {
        pos: usize,
    },
    // Nested structures whitch use stack to store and parse
    ReadingArray {
        pos: usize,
        total: usize,
        current: usize,
        elements: Vec<RespValue<'static>>,
    },
    // Outcomes
    Error(ParseError),
    Complete(Option<(RespValue<'static>, usize)>),
}

#[derive(Debug, Clone)]
pub struct Parser {
    pub buffer: BytesMut,
    state: ParseState,
    max_length: usize,
    max_depth: usize,
    nested_stack: Vec<ParseState>,
}

/// A parser for RESP (REdis Serialization Protocol) messages.
///
/// # Example
///
/// ```
/// use stream_resp::parser::Parser;
/// use stream_resp::resp::RespValue;
///
/// let mut parser = Parser::new(10, 1024);
/// parser.read_buf(b"+OK\r\n");
/// let result = parser.try_parse();
/// assert_eq!(result.unwrap(), Some(RespValue::SimpleString("OK".into())));
/// ```
///
/// # Methods
///
/// - `new(max_depth: usize, max_length: usize) -> Self`
///   Creates a new `Parser` instance with the specified maximum depth and length.
///
/// - `read_buf(&mut self, buf: &[u8])`
///   Reads a buffer of bytes into the parser's internal buffer.
///
/// - `get_buffer(&self) -> &BytesMut`
///   Returns a reference to the parser's internal buffer.
///
/// - `clear_buffer(&mut self)`
///   Clears the parser's internal buffer and resets the state.
///
/// - `try_parse(&mut self) -> ParseResult`
///   Attempts to parse the data in the buffer and returns a `ParseResult`.
///
/// # Internal Methods
///
/// - `find_crlf(&self, start: usize) -> Option<usize>`
///   Finds the position of the CRLF sequence starting from the given position.
///
/// - `handle_index(&mut self, index: usize) -> ParseState`
///   Handles the initial parsing state based on the type marker at the given index.
///
/// - `handle_length(&mut self, pos: usize, value: i64, negative: bool, type_char: u8) -> ParseState`
///   Handles the parsing of length-prefixed types (bulk strings and arrays).
///
/// - `handle_bulk_string(&mut self, start_pos: usize, remaining: usize) -> ParseState`
///   Handles the parsing of bulk strings.
///
/// - `handle_array(&mut self, pos: usize, total: usize, current: usize, elements: Vec<RespValue<'static>>) -> ParseState`
///   Handles the parsing of arrays.
///
/// - `handle_simple_string(&mut self, pos: usize) -> ParseState`
///   Handles the parsing of simple strings.
///
/// - `handle_error(&mut self, pos: usize) -> ParseState`
///   Handles the parsing of error messages.
///
/// - `handle_integer(&mut self, pos: usize) -> ParseState`
///   Handles the parsing of integer values.
impl Parser {
    /// Creates a new parser instance.
    ///
    /// # Arguments
    ///
    /// * `max_depth` - The maximum depth of nested arrays.
    /// * `max_length` - The maximum length of bulk strings.
    ///
    /// # Returns
    ///
    /// Returns a new `Parser` instance.
    pub fn new(max_depth: usize, max_length: usize) -> Self {
        Parser {
            buffer: BytesMut::with_capacity(DEFAULT_BUFFER_INIT_SIZE),
            state: ParseState::Index { pos: 0 },
            max_length,
            max_depth,
            nested_stack: Vec::with_capacity(max_depth),
        }
    }

    pub fn read_buf(&mut self, buf: &[u8]) {
        // Create more efficient sliding window buffer
        if self.buffer.len() > 0 && self.buffer.capacity() < self.buffer.len() + buf.len() {
            // If we've processed part of the data, we can keep the unprocessed part
            if let ParseState::Index { pos } = self.state {
                if pos > 0 {
                    // Create a new buffer with the remaining data
                    let remaining = self.buffer.split_off(pos);
                    self.buffer = remaining;
                    self.state = ParseState::Index { pos: 0 };
                }
            }
        }

        // If the buffer is still too small, consider clearing it
        if self.buffer.capacity() < buf.len() {
            self.buffer.clear();
            self.buffer.reserve(buf.len() + DEFAULT_BUFFER_INIT_SIZE);
        }

        self.buffer.extend_from_slice(buf);
    }

    /// Returns a reference to the parser's internal buffer.
    ///
    /// # Returns
    ///
    /// A reference to the internal buffer.
    pub fn buffer(&self) -> &BytesMut {
        &self.buffer
    }

    #[inline(always)]
    fn find_crlf(&self, start: usize) -> Option<usize> {
        // Use memchr's more optimized implementation
        let buf = &self.buffer[start..];
        let r_position = memchr(b'\r', buf)?;
        let pos = start + r_position;

        // Check if there's a \n after the \r
        if pos + 1 < self.buffer.len() && self.buffer[pos + 1] == b'\n' {
            Some(pos)
        } else {
            // Keep searching past this \r
            self.find_crlf(pos + 1)
        }
    }

    #[inline(always)]
    fn handle_index(&mut self, index: usize) -> ParseState {
        if index >= self.buffer.len() {
            return ParseState::Error(ParseError::UnexpectedEof);
        }

        match self.buffer[index] {
            b'+' => ParseState::ReadingSimpleString { pos: index + 1 },
            b'-' => ParseState::ReadingError { pos: index + 1 },
            b':' => ParseState::ReadingInteger { pos: index + 1 },
            b'$' => ParseState::ReadingLength {
                value: 0,
                negative: false,
                pos: index + 1,
                type_char: b'$',
            },
            b'*' => ParseState::ReadingLength {
                value: 0,
                negative: false,
                pos: index + 1,
                type_char: b'*',
            },
            b'\r' => {
                // Handle CRLF for array elements
                if index + 1 < self.buffer.len() && self.buffer[index + 1] == b'\n' {
                    ParseState::Index { pos: index + 2 }
                } else {
                    ParseState::Error(ParseError::InvalidFormat("Expected \\n after \\r".into()))
                }
            }
            _ => ParseState::Error(ParseError::InvalidFormat("Invalid type marker".into())),
        }
    }

    #[inline(always)]
    fn handle_length(
        &mut self,
        pos: usize,
        value: i64,
        negative: bool,
        type_char: u8,
    ) -> ParseState {
        return match self.buffer.get(pos) {
            Some(&b) => match b {
                b'0'..=b'9' => {
                    let new_value = match value.checked_mul(10).and_then(|v| {
                        if negative {
                            v.checked_sub((b - b'0') as i64)
                        } else {
                            v.checked_add((b - b'0') as i64)
                        }
                    }) {
                        Some(v) => v,
                        None => {
                            return ParseState::Error(ParseError::Overflow);
                        }
                    };

                    ParseState::ReadingLength {
                        pos: pos + 1,
                        value: new_value,
                        negative,
                        type_char,
                    }
                }
                b'-' => ParseState::ReadingLength {
                    pos: pos + 1,
                    value,
                    negative: true,
                    type_char,
                },
                b'\r' => match self.buffer.get(pos + 1) {
                    Some(&b'\n') => match type_char {
                        b'$' => {
                            if value <= 0 {
                                ParseState::Complete(Some((RespValue::Null, pos)))
                            } else {
                                ParseState::ReadingBulkString {
                                    start_pos: pos + 2,
                                    remaining: value as usize,
                                }
                            }
                        }
                        b'*' => {
                            if value <= 0 {
                                ParseState::Complete(Some((RespValue::Array(None), pos)))
                            } else {
                                ParseState::ReadingArray {
                                    pos: pos + 2,
                                    total: value as usize,
                                    elements: Vec::with_capacity(value as usize),
                                    current: 1,
                                }
                            }
                        }
                        b':' => ParseState::Complete(Some((RespValue::Integer(value), pos))),
                        _ => ParseState::Error(ParseError::InvalidFormat(
                            "Invalid length type".into(),
                        )),
                    },
                    _ => ParseState::Error(ParseError::InvalidFormat(
                        "Expected \\n after \\r".into(),
                    )),
                },
                _ => ParseState::Error(ParseError::InvalidFormat(
                    "Invalid character in length".into(),
                )),
            },
            None => ParseState::Error(ParseError::UnexpectedEof),
        };
    }

    #[inline(always)]
    fn handle_bulk_string(&mut self, start_pos: usize, remaining: usize) -> ParseState {
        // Early returns for special cases
        if remaining == 0 {
            return ParseState::Complete(Some((RespValue::BulkString(None), start_pos + CRLF_LEN)));
        }

        if remaining >= self.max_length {
            return ParseState::Error(ParseError::InvalidLength);
        }

        let required_len = start_pos + remaining + CRLF_LEN;
        if self.buffer.len() < required_len {
            return ParseState::Error(ParseError::NotEnoughData);
        }

        // Check terminator first to fail fast
        if self.buffer[start_pos + remaining] != b'\r'
            || self.buffer[start_pos + remaining + 1] != b'\n'
        {
            return ParseState::Error(ParseError::InvalidFormat("Missing CRLF terminator".into()));
        }

        // Create string view
        let string_slice = &self.buffer[start_pos..start_pos + remaining];

        // Optimize ASCII check
        let is_ascii = string_slice.iter().all(|&b| b < 128);

        // Build result efficiently based on content type
        let result = if is_ascii {
            // Fast path for ASCII
            let s = unsafe { std::str::from_utf8_unchecked(string_slice) }.to_string();
            RespValue::BulkString(Some(Cow::Owned(s)))
        } else {
            // Only do UTF-8 validation for non-ASCII
            match std::str::from_utf8(string_slice) {
                Ok(s) => RespValue::BulkString(Some(Cow::Owned(s.to_string()))),
                Err(_) => return ParseState::Error(ParseError::InvalidUtf8),
            }
        };

        ParseState::Complete(Some((result, start_pos + remaining + CRLF_LEN)))
    }

    #[inline(always)]
    fn handle_array(
        &mut self,
        pos: usize,
        total: usize,
        current: usize,
        elements: Vec<RespValue<'static>>,
    ) -> ParseState {
        if total == 0 {
            return ParseState::Complete(Some((RespValue::Array(None), pos)));
        }

        if current > total {
            return ParseState::Complete(Some((RespValue::Array(Some(elements)), pos)));
        }

        // Store current array state
        self.nested_stack.push(ParseState::ReadingArray {
            pos,
            total,
            current,
            elements,
        });

        // Start parsing next element from current position
        ParseState::Index { pos }
    }

    #[inline(always)]
    fn handle_simple_string(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = &self.buffer[pos..end_pos];

                // Use from_utf8_lossy to directly create Cow<str>
                let string = String::from_utf8_lossy(bytes).into_owned();

                ParseState::Complete(Some((
                    RespValue::SimpleString(Cow::Owned(string)),
                    end_pos + CRLF_LEN,
                )))
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    #[inline(always)]
    fn handle_error(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = &self.buffer[pos..end_pos];

                // Use from_utf8_lossy to directly create Cow<str>
                let error = String::from_utf8_lossy(bytes).into_owned();

                ParseState::Complete(Some((
                    RespValue::Error(Cow::Owned(error)),
                    end_pos + CRLF_LEN,
                )))
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    #[inline(always)]
    fn handle_integer(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = &self.buffer[pos..end_pos];

                // 小整数快速路径
                if bytes.len() <= 10 {
                    let mut value: i64 = 0;
                    let mut start = 0;
                    let negative = bytes.first() == Some(&b'-');

                    if negative {
                        start = 1;
                    }

                    for &byte in &bytes[start..] {
                        if byte < b'0' || byte > b'9' {
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Invalid integer".into(),
                            ));
                        }
                        if value > i64::MAX / 10 {
                            return ParseState::Error(ParseError::Overflow);
                        }
                        value = value * 10 + (byte - b'0') as i64;
                    }

                    return ParseState::Complete(Some((
                        RespValue::Integer(if negative { -value } else { value }),
                        end_pos + CRLF_LEN,
                    )));
                }

                match atoi::atoi::<i64>(bytes) {
                    Some(value) => {
                        ParseState::Complete(Some((RespValue::Integer(value), end_pos + CRLF_LEN)))
                    }
                    None => ParseState::Error(ParseError::InvalidFormat("Invalid integer".into())),
                }
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    /// Clears the parser's internal buffer and resets the state.
    pub fn clear_buffer(&mut self, pos: usize) {
        self.state = ParseState::Index { pos };
        self.nested_stack.clear();
    }

    /// Attempts to parse the data in the buffer and returns a `ParseResult`.
    ///
    /// This method will iterate through the buffer, checking for maximum iterations and depth.
    ///
    /// # Returns
    ///
    /// Returns a `ParseResult` which is either a `RespValue` or a `ParseError`.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::InvalidFormat` if the maximum number of iterations is exceeded.
    /// Returns `ParseError::InvalidDepth` if the maximum nested depth is exceeded.
    pub fn try_parse(&mut self) -> ParseResult {
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return Err(ParseError::InvalidFormat(
                    "Maximum parsing iterations exceeded".into(),
                ));
            }

            // Check max Depth
            if self.nested_stack.len() > self.max_depth {
                return Err(ParseError::InvalidDepth);
            }

            debug!(
                "{:?} | state={:?} | buffer={:?} | nested_len:{:?}",
                iterations,
                self.state,
                String::from_utf8_lossy(&self.buffer),
                self.nested_stack.len()
            );

            let current_state = self.state.clone();
            let next_state = match current_state {
                ParseState::Index { pos } => self.handle_index(pos),
                ParseState::ReadingArray {
                    pos,
                    total,
                    current,
                    elements,
                } => self.handle_array(pos, total, current, elements),
                ParseState::ReadingLength {
                    pos,
                    value,
                    negative,
                    type_char,
                } => self.handle_length(pos, value, negative, type_char),
                ParseState::ReadingBulkString {
                    start_pos,
                    remaining,
                } => self.handle_bulk_string(start_pos, remaining),
                ParseState::ReadingSimpleString { pos } => self.handle_simple_string(pos),
                ParseState::ReadingError { pos } => self.handle_error(pos),
                ParseState::ReadingInteger { pos } => self.handle_integer(pos),
                ParseState::Error(error) => ParseState::Error(error),
                ParseState::Complete(value) => ParseState::Complete(value),
            };

            match next_state {
                ParseState::Complete(Some((value, pos))) => match self.nested_stack.last_mut() {
                    Some(ParseState::ReadingArray {
                        total,
                        elements,
                        current,
                        ..
                    }) => {
                        elements.push(value);

                        if *current < *total {
                            *current += 1;
                            self.state = ParseState::Index { pos };
                            continue;
                        } else {
                            let mut elements_vec = Vec::new();
                            if let ParseState::ReadingArray {
                                elements: ref mut arr_elements,
                                ..
                            } = self.nested_stack.last_mut().unwrap()
                            {
                                std::mem::swap(arr_elements, &mut elements_vec);
                            }

                            let completed_result = RespValue::Array(Some(elements_vec));
                            if !self.nested_stack.is_empty() {
                                self.nested_stack.pop();
                                self.state = ParseState::Complete(Some((completed_result, pos)));
                                continue;
                            } else {
                                self.clear_buffer(pos);
                                if completed_result.is_none() {
                                    self.state = ParseState::Complete(None);
                                } else {
                                    return Ok(Some(completed_result));
                                }
                            }
                        }
                    }
                    _ => {
                        if self.nested_stack.is_empty() {
                            self.clear_buffer(pos);
                            return Ok(Some(value));
                        }
                    }
                },
                ParseState::Error(error) => {
                    return Err(error);
                }
                _ => self.state = next_state,
            }
        }
    }
}

//EOF
