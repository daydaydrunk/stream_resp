use crate::resp::RespValue;
use bytes::BytesMut;
use std::borrow::Cow;
use tracing::debug;

const MAX_ITERATIONS: usize = 128;
const CRLF_LEN: usize = 2;
const BUFFER_INIT_SIZE: usize = 4096;
const CR: u8 = b'\r';
const LF: u8 = b'\n';
const NEXT: usize = 1;
const NO_REMAINING: usize = 0;

type ParseResult = Result<Option<RespValue<'static>>, ParseError>;

#[derive(Debug, PartialEq, Clone)]
pub enum ParseError {
    InvalidFormat(Cow<'static, str>),
    InvalidLength,
    UnexpectedEof,
    Overflow,
    NotEnoughData,
    InvalidDepth,
}

#[derive(Debug, PartialEq, Clone)]
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

impl Parser {
    pub fn new(max_depth: usize, max_length: usize) -> Self {
        Parser {
            buffer: BytesMut::with_capacity(BUFFER_INIT_SIZE),
            state: ParseState::Index { pos: 0 },
            max_length,
            max_depth,
            nested_stack: Vec::with_capacity(max_depth),
        }
    }

    pub fn read_buf(&mut self, buf: &[u8]) {
        self.buffer.extend_from_slice(buf);
    }

    pub fn get_buffer(&self) -> &BytesMut {
        &self.buffer
    }

    #[inline]
    fn find_crlf(&self, start: usize) -> Option<usize> {
        let mut pos = start;
        while pos < self.buffer.len().saturating_sub(1) {
            match (self.buffer.get(pos), self.buffer.get(pos + 1)) {
                (Some(&b'\r'), Some(&b'\n')) => return Some(pos),
                (Some(_), _) => pos += 1,
                _ => break,
            }
        }
        None
    }

    #[inline]
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

    #[inline]
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

    #[inline]
    fn handle_bulk_string(&mut self, start_pos: usize, remaining: usize) -> ParseState {
        if remaining == 0 {
            return ParseState::Complete(Some((RespValue::BulkString(None), 0)));
        }

        if remaining >= self.max_length {
            return ParseState::Error(ParseError::InvalidLength);
        } else if remaining == NO_REMAINING {
            return ParseState::Complete(Some((RespValue::BulkString(None), start_pos)));
        }

        let required_len = start_pos + remaining + CRLF_LEN;
        if self.buffer.len() < required_len {
            return ParseState::Error(ParseError::NotEnoughData);
        }

        if self.buffer[start_pos + remaining] != CR
            || self.buffer[start_pos + remaining + NEXT] != LF
        {
            return ParseState::Error(ParseError::InvalidFormat("Missing CRLF".into()));
        }

        match String::from_utf8(self.buffer[start_pos..start_pos + remaining].to_vec()) {
            Ok(content) => ParseState::Complete(Some((
                RespValue::BulkString(Some(content.into())),
                required_len,
            ))),
            Err(_) => ParseState::Error(ParseError::InvalidFormat("Invalid UTF-8".into())),
        }
    }

    #[inline]
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
        let arr = ParseState::ReadingArray {
            pos,
            total,
            elements,
            current,
        };

        self.nested_stack.push(arr);

        // Start parsing next element from current position
        ParseState::Index { pos }
    }

    #[inline]
    fn handle_simple_string(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = self.buffer[pos..end_pos].to_vec();
                let string = String::from_utf8_lossy(&bytes).into_owned().into();
                ParseState::Complete(Some((RespValue::SimpleString(string), end_pos)))
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    #[inline]
    fn handle_error(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let bytes = self.buffer[pos..end_pos].to_vec();
                let string = String::from_utf8_lossy(&bytes).into_owned().into();
                ParseState::Complete(Some((RespValue::Error(string), end_pos)))
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    #[inline]
    fn handle_integer(&mut self, pos: usize) -> ParseState {
        match self.find_crlf(pos) {
            Some(end_pos) => {
                let mut value = 0i64;
                let mut negative = false;
                let mut start = pos;

                match self.buffer.get(pos) {
                    Some(&b'-') => {
                        negative = true;
                        start = pos + 1;
                    }
                    _ => {}
                }

                for &b in &self.buffer[start..end_pos] {
                    match b {
                        b'0'..=b'9' => {
                            value = match value.checked_mul(10).and_then(|v| {
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
                        }
                        _ => {
                            return ParseState::Error(ParseError::InvalidFormat(
                                "Invalid integer format".into(),
                            ));
                        }
                    }
                }
                ParseState::Complete(Some((RespValue::Integer(value), end_pos + CRLF_LEN)))
            }
            None => ParseState::Error(ParseError::UnexpectedEof),
        }
    }

    #[inline]
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.state = ParseState::Index { pos: 0 };
    }

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

            let next_state = match &self.state {
                ParseState::Index { pos } => self.handle_index(*pos),
                ParseState::ReadingArray {
                    pos,
                    total,
                    elements,
                    current,
                } => self.handle_array(*pos, *total, *current, elements.clone()),
                ParseState::ReadingLength {
                    pos,
                    value,
                    negative,
                    type_char,
                } => self.handle_length(*pos, *value, *negative, *type_char),
                ParseState::ReadingBulkString {
                    start_pos,
                    remaining,
                } => self.handle_bulk_string(*start_pos, *remaining),
                ParseState::ReadingSimpleString { pos } => self.handle_simple_string(*pos),
                ParseState::ReadingError { pos } => self.handle_error(*pos),
                ParseState::ReadingInteger { pos } => self.handle_integer(*pos),
                ParseState::Error(error) => ParseState::Error(error.clone()),
                ParseState::Complete(value) => ParseState::Complete(value.clone()),
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
                            let completed_result = RespValue::Array(Some(elements.clone()));
                            if !self.nested_stack.is_empty() {
                                self.nested_stack.pop();
                                self.state = ParseState::Complete(Some((completed_result, pos)));
                                continue;
                            } else {
                                self.clear_buffer();
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
                            self.clear_buffer();
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
