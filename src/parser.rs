use crate::resp::RespValue;
use bytes::BytesMut; // Add Buf trait
use memchr::memchr;
use std::borrow::Cow;
use std::fmt; // Import fmt
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

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            ParseError::InvalidLength => write!(f, "Invalid length"),
            ParseError::UnexpectedEof => write!(f, "Unexpected end of input"),
            ParseError::Overflow => write!(f, "Numeric overflow"),
            ParseError::NotEnoughData => write!(f, "Not enough data in buffer"),
            ParseError::InvalidDepth => write!(f, "Maximum nesting depth exceeded"),
            ParseError::InvalidUtf8 => write!(f, "Invalid UTF-8 sequence"),
        }
    }
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
        original_type_char: u8, // Added to distinguish between Array (*) and Map (%)
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
/// - `handle_array(&mut self, pos: usize, total: usize, current: usize, elements: Vec<RespValue<'static>>, original_type_char: u8) -> ParseState`
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
            b'%' => ParseState::ReadingLength {
                // Added Map type marker
                value: 0,
                negative: false,
                pos: index + 1,
                type_char: b'%',
            },
            b'~' => ParseState::ReadingLength {
                // Added Set type marker
                value: 0,
                negative: false,
                pos: index + 1,
                type_char: b'~',
            },
            b'>' => ParseState::ReadingLength {
                // Added Push type marker
                value: 0,
                negative: false,
                pos: index + 1,
                type_char: b'>',
            },
            b'_' => {
                // Handle Null type
                if index + 2 < self.buffer.len()
                    && self.buffer[index + 1] == b'\r'
                    && self.buffer[index + 2] == b'\n'
                {
                    ParseState::Complete(Some((RespValue::Null, index + 3)))
                } else {
                    ParseState::Error(ParseError::UnexpectedEof)
                }
            }
            b'#' => {
                // Handle Boolean type
                if index + 2 < self.buffer.len()
                    && self.buffer[index + 2] == b'\r'
                    && index + 3 < self.buffer.len()
                    && self.buffer[index + 3] == b'\n'
                {
                    match self.buffer[index + 1] {
                        b't' => ParseState::Complete(Some((RespValue::Boolean(true), index + 4))),
                        b'f' => ParseState::Complete(Some((RespValue::Boolean(false), index + 4))),
                        _ => ParseState::Error(ParseError::InvalidFormat(
                            "Invalid boolean value".into(),
                        )),
                    }
                } else {
                    ParseState::Error(ParseError::UnexpectedEof)
                }
            }
            b',' => {
                // Handle Double type
                match self.find_crlf(index + 1) {
                    Some(end_pos) => {
                        let bytes = &self.buffer[(index + 1)..end_pos];
                        let double_str = std::str::from_utf8(bytes);

                        match double_str {
                            Ok(s) => match s.parse::<f64>() {
                                Ok(value) => ParseState::Complete(Some((
                                    RespValue::Double(value),
                                    end_pos + CRLF_LEN,
                                ))),
                                Err(_) => ParseState::Error(ParseError::InvalidFormat(
                                    "Invalid double value".into(),
                                )),
                            },
                            Err(_) => ParseState::Error(ParseError::InvalidUtf8),
                        }
                    }
                    None => ParseState::Error(ParseError::UnexpectedEof),
                }
            }
            b'(' => {
                // Handle Big Number type
                match self.find_crlf(index + 1) {
                    Some(end_pos) => {
                        let bytes = &self.buffer[(index + 1)..end_pos];

                        // Verify that the big number contains only valid characters (digits and optional leading minus)
                        let is_valid = bytes
                            .iter()
                            .enumerate()
                            .all(|(i, &b)| (b'0'..=b'9').contains(&b) || (i == 0 && b == b'-'));

                        if !is_valid {
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Invalid big number format".into(),
                            ));
                        }

                        match std::str::from_utf8(bytes) {
                            Ok(s) => ParseState::Complete(Some((
                                RespValue::BigNumber(Cow::Owned(s.to_string())),
                                end_pos + CRLF_LEN,
                            ))),
                            Err(_) => ParseState::Error(ParseError::InvalidUtf8),
                        }
                    }
                    None => ParseState::Error(ParseError::UnexpectedEof),
                }
            }
            b'!' => {
                // Handle Bulk Error type
                match self.find_crlf(index + 1) {
                    Some(end_pos) => {
                        let bytes = &self.buffer[(index + 1)..end_pos];

                        // Check for null bulk error (-1)
                        if bytes.len() == 2 && bytes[0] == b'-' && bytes[1] == b'1' {
                            return ParseState::Complete(Some((
                                RespValue::BulkError(None),
                                end_pos + CRLF_LEN,
                            )));
                        }

                        match std::str::from_utf8(bytes) {
                            Ok(s) => ParseState::Complete(Some((
                                RespValue::BulkError(Some(Cow::Owned(s.to_string()))),
                                end_pos + CRLF_LEN,
                            ))),
                            Err(_) => ParseState::Error(ParseError::InvalidUtf8),
                        }
                    }
                    None => ParseState::Error(ParseError::UnexpectedEof),
                }
            }
            b'=' => {
                // Handle Verbatim String type
                match self.find_crlf(index + 1) {
                    Some(end_pos) => {
                        let bytes = &self.buffer[(index + 1)..end_pos];

                        // Check for null verbatim string (-1)
                        if bytes.len() == 2 && bytes[0] == b'-' && bytes[1] == b'1' {
                            return ParseState::Complete(Some((
                                RespValue::VerbatimString(None),
                                end_pos + CRLF_LEN,
                            )));
                        }

                        match std::str::from_utf8(bytes) {
                            Ok(s) => ParseState::Complete(Some((
                                RespValue::VerbatimString(Some(Cow::Owned(s.to_string()))),
                                end_pos + CRLF_LEN,
                            ))),
                            Err(_) => ParseState::Error(ParseError::InvalidUtf8),
                        }
                    }
                    None => ParseState::Error(ParseError::UnexpectedEof),
                }
            }
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
                    Some(&b'\n') => {
                        let next_pos = pos + CRLF_LEN; // Position after CRLF
                        match type_char {
                            b'$' => {
                                if value < 0 {
                                    // RESP3 Null Bulk String $-1\r\n
                                    ParseState::Complete(Some((
                                        RespValue::BulkString(None),
                                        next_pos,
                                    )))
                                } else if value == 0 {
                                    // RESP3 Empty Bulk String $0\r\n\r\n
                                    // Need to check for the second CRLF
                                    if self.buffer.len() >= next_pos + CRLF_LEN
                                        && self.buffer[next_pos..next_pos + CRLF_LEN] == *b"\r\n"
                                    {
                                        ParseState::Complete(Some((
                                            RespValue::BulkString(Some(Cow::Borrowed(""))),
                                            next_pos + CRLF_LEN,
                                        )))
                                    } else {
                                        ParseState::Error(ParseError::UnexpectedEof) // Or NotEnoughData
                                    }
                                } else {
                                    ParseState::ReadingBulkString {
                                        start_pos: next_pos,
                                        remaining: value as usize,
                                    }
                                }
                            }
                            b'*' | b'%' | b'~' | b'>' => {
                                // Handle Array, Map, Set, Push length
                                if value < 0 {
                                    // RESP3 Null Aggregate Type
                                    let null_value = match type_char {
                                        b'*' => RespValue::Array(None),
                                        b'%' => RespValue::Map(None),
                                        b'~' => RespValue::Set(None),
                                        b'>' => RespValue::Push(None),
                                        _ => unreachable!(), // Should be covered by outer match
                                    };
                                    ParseState::Complete(Some((null_value, next_pos)))
                                } else if value == 0 {
                                    // RESP3 Empty Aggregate Type
                                    let empty_value = match type_char {
                                        b'*' => RespValue::Array(Some(vec![])),
                                        b'%' => RespValue::Map(Some(vec![])),
                                        b'~' => RespValue::Set(Some(vec![])),
                                        b'>' => RespValue::Push(Some(vec![])),
                                        _ => unreachable!(),
                                    };
                                    ParseState::Complete(Some((empty_value, next_pos)))
                                } else {
                                    let total_elements = if type_char == b'%' {
                                        (value * 2) as usize // Maps have key-value pairs
                                    } else {
                                        value as usize
                                    };
                                    ParseState::ReadingArray {
                                        // Use ReadingArray for all aggregate types
                                        pos: next_pos,
                                        total: total_elements,
                                        elements: Vec::with_capacity(total_elements),
                                        current: 0, // Start counting from 0 elements read
                                        original_type_char: type_char, // Store the original type
                                    }
                                }
                            }
                            b':' => {
                                ParseState::Complete(Some((RespValue::Integer(value), next_pos)))
                            }
                            _ => ParseState::Error(ParseError::InvalidFormat(
                                "Invalid length type".into(),
                            )),
                        }
                    }
                    _ => ParseState::Error(ParseError::InvalidFormat(
                        "Expected \\n after \\r".into(),
                    )),
                },
                _ => ParseState::Error(ParseError::InvalidFormat(
                    "Invalid character in length".into(),
                )),
            },
            None => ParseState::Error(ParseError::UnexpectedEof), // Changed from NotEnoughData
        };
    }

    #[inline(always)]
    fn handle_bulk_string(&mut self, start_pos: usize, remaining: usize) -> ParseState {
        // Early returns for special cases
        if remaining == 0 {
            // This case should ideally not be reached if handle_length handles $0 correctly.
            // If it is reached, it implies an empty string content followed by CRLF.
            // Let's treat it as an error or unexpected state for now.
            return ParseState::Error(ParseError::InvalidFormat(
                "Unexpected zero remaining in handle_bulk_string".into(),
            ));
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
        original_type_char: u8, // Pass original_type_char
    ) -> ParseState {
        if current >= total {
            // Check if all elements are read
            // Completion logic moved to try_parse
            // This state should only transition to Index or Error here
            // If we reach here, it means we are ready to parse the next element
            ParseState::Index { pos }
        } else {
            // Store current array/map state
            self.nested_stack.push(ParseState::ReadingArray {
                pos, // Position *after* the element we just parsed
                total,
                current, // Number of elements *already* parsed
                elements,
                original_type_char,
            });

            // Start parsing next element from current position
            ParseState::Index { pos }
        }
    }

    #[inline(always)]
    fn handle_simple_string(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = &self.buffer[pos..end_pos];

                // Validate no CR/LF in simple strings per RESP3 spec
                if bytes.iter().any(|&b| b == b'\r' || b == b'\n') {
                    return ParseState::Error(ParseError::InvalidFormat(
                        "Simple string cannot contain CR or LF".into(),
                    ));
                }

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

                // Check for explicit plus sign
                let explicit_plus = bytes.first() == Some(&b'+');

                if explicit_plus {
                    #[cfg(feature = "explicit-positive-sign")]
                    {
                        // If feature enabled, skip the '+' and parse the rest
                        bytes = &bytes[1..];
                        if bytes.is_empty() {
                            // Handle case like ":+\r\n"
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Invalid integer format after '+'".into(),
                            ));
                        }
                    }
                    #[cfg(not(feature = "explicit-positive-sign"))]
                    {
                        // If feature disabled, '+' is invalid
                        return ParseState::Error(ParseError::InvalidFormat(
                            "Explicit '+' sign in integer not supported (use 'explicit-positive-sign' feature)".into(),
                        ));
                    }
                }

                // 小整数快速路径 (Small integer fast path)
                // Use the potentially modified 'bytes' slice
                if bytes.len() <= 19 {
                    // Adjusted length check slightly for safety with i64
                    let mut value: i64 = 0;
                    let mut start = 0;
                    let negative = bytes.first() == Some(&b'-');

                    if negative {
                        // Cannot have both explicit '+' and '-'
                        if explicit_plus {
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Cannot have both '+' and '-' signs in integer".into(),
                            ));
                        }
                        start = 1;
                    }

                    if start >= bytes.len() && (negative || explicit_plus) {
                        // Handle cases like ":-\r\n" or ":+\r\n" (if feature enabled)
                        return ParseState::Error(ParseError::InvalidFormat(
                            "Invalid integer format after sign".into(),
                        ));
                    }

                    for &byte in &bytes[start..] {
                        if !(b'0'..=b'9').contains(&byte) {
                            // Simplified check
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Invalid character in integer".into(),
                            ));
                        }
                        // Check for potential overflow before multiplication
                        if value > (i64::MAX - (byte - b'0') as i64) / 10 {
                            return ParseState::Error(ParseError::Overflow);
                        }
                        value = value * 10 + (byte - b'0') as i64;
                    }

                    // Apply sign if negative
                    if negative {
                        // Check for potential overflow for i64::MIN
                        if value == i64::MAX.wrapping_add(1) {
                            // Check if value represents abs(i64::MIN)
                            value = i64::MIN; // Assign directly to avoid negation overflow
                        } else {
                            value = -value;
                        }
                    }

                    return ParseState::Complete(Some((
                        RespValue::Integer(value),
                        end_pos + CRLF_LEN,
                    )));
                }

                // Fallback to atoi for potentially larger strings (or if fast path logic needs refinement)
                // Note: atoi might handle '+' differently or not at all depending on its implementation.
                // If using atoi, ensure its behavior aligns with the feature flag expectation.
                // The fast path above is generally preferred for RESP integers.
                match atoi::atoi::<i64>(bytes) {
                    Some(value) => {
                        // If explicit_plus was handled and atoi doesn't support '+', this might be redundant
                        // or needs adjustment based on atoi behavior.
                        // Assuming atoi handles '-' correctly but maybe not '+'.
                        #[cfg(feature = "explicit-positive-sign")]
                        {
                            // If atoi parsed successfully, it should be the correct value
                            ParseState::Complete(Some((
                                RespValue::Integer(value),
                                end_pos + CRLF_LEN,
                            )))
                        }
                        #[cfg(not(feature = "explicit-positive-sign"))]
                        {
                            // If '+' was present, we should have errored earlier.
                            // If '-' or no sign, atoi result is fine.
                            if explicit_plus {
                                // This path shouldn't be reached if '+' is invalid
                                ParseState::Error(ParseError::InvalidFormat(
                                    "Internal error: explicit '+' parsed unexpectedly".into(),
                                ))
                            } else {
                                ParseState::Complete(Some((
                                    RespValue::Integer(value),
                                    end_pos + CRLF_LEN,
                                )))
                            }
                        }
                    }
                    None => ParseState::Error(ParseError::InvalidFormat(
                        "Invalid integer format (atoi failed)".into(),
                    )),
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
                    original_type_char, // Pass to handler
                } => self.handle_array(pos, total, current, elements, original_type_char),
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
                ParseState::Complete(Some((value, pos))) => {
                    // Check if we are inside a nested structure (Array or Map)
                    if let Some(ParseState::ReadingArray {
                        total,
                        elements,
                        current,
                        ..
                    }) = self.nested_stack.last_mut()
                    {
                        elements.push(value);
                        *current += 1;

                        if *current < *total {
                            // More elements needed for this array/map, continue parsing from `pos`
                            self.state = ParseState::Index { pos };
                            continue;
                        } else {
                            // Array/Map/Set/Push is complete, pop it from the stack
                            let mut completed_elements = Vec::new();
                            let finished_type_char: u8;

                            // Pop the completed ReadingArray state
                            if let Some(ParseState::ReadingArray {
                                elements: final_elements,
                                original_type_char: type_char,
                                ..
                            }) = self.nested_stack.pop()
                            {
                                completed_elements = final_elements;
                                finished_type_char = type_char;
                            } else {
                                // Should not happen if logic is correct
                                return Err(ParseError::InvalidFormat(
                                    "Mismatched nested stack state".into(),
                                ));
                            }

                            // Construct the final value (Array, Map, Set, or Push)
                            let completed_result = match finished_type_char {
                                b'%' => {
                                    // Map
                                    let mut map_pairs =
                                        Vec::with_capacity(completed_elements.len() / 2);
                                    let mut iter = completed_elements.into_iter();
                                    while let (Some(key), Some(val)) = (iter.next(), iter.next()) {
                                        map_pairs.push((key, val));
                                    }
                                    RespValue::Map(Some(map_pairs))
                                }
                                b'~' => {
                                    // Set
                                    RespValue::Set(Some(completed_elements))
                                }
                                b'>' => {
                                    // Push
                                    RespValue::Push(Some(completed_elements))
                                }
                                _ => {
                                    // Default to Array (*)
                                    RespValue::Array(Some(completed_elements))
                                }
                            };

                            // If the stack is now empty, this is the final result
                            if self.nested_stack.is_empty() {
                                self.clear_buffer(pos);
                                return Ok(Some(completed_result));
                            } else {
                                // Otherwise, this completed structure is an element of the parent structure
                                // Push it back onto the parent's state (which is now on top of the stack)
                                // The loop will continue and handle pushing this `completed_result`
                                self.state = ParseState::Complete(Some((completed_result, pos)));
                                continue; // Re-evaluate with the completed value in the next iteration
                            }
                        }
                    } else {
                        // Not in a nested structure, this is the final result
                        if self.nested_stack.is_empty() {
                            self.clear_buffer(pos);
                            return Ok(Some(value));
                        } else {
                            // This case might indicate an issue, e.g., completing a value when stack isn't empty but top isn't ReadingArray
                            return Err(ParseError::InvalidFormat(
                                "Unexpected completion state".into(),
                            ));
                        }
                    }
                }
                ParseState::Complete(None) => {
                    // Handle cases like Null Array/Map completion if needed
                    // This might occur if a Null Array/Map is parsed directly
                    if self.nested_stack.is_empty() {
                        // Assuming pos is correctly set by the state that produced Complete(None)
                        // Need to ensure states like handle_length correctly set pos for null/empty types
                        // Let's assume the pos is advanced correctly by the caller state
                        self.state = ParseState::Index { pos: 0 }; // Reset or use appropriate pos
                        return Ok(None); // Indicate no complete value parsed (e.g. null array)
                    } else {
                        // Handle null/empty completion within a nested structure if necessary
                        // This part might need refinement based on how Complete(None) is generated
                        return Err(ParseError::InvalidFormat(
                            "Unexpected None completion in nested structure".into(),
                        ));
                    }
                }
                ParseState::Error(error) => {
                    return Err(error);
                }
                // Any other state just becomes the current state for the next iteration
                _ => self.state = next_state,
            }
        }
    }
}

//EOF
