use std::borrow::Cow;
use stream_resp::parser::{ParseError, Parser};
use stream_resp::resp::RespValue;

fn main() {
    {
        let mut parser = Parser::new(100, 1000);

        // First chunk: type marker
        parser.read_buf(b"$5");
        let result = parser.try_parse();
        assert_eq!(result, Err(ParseError::UnexpectedEof));

        // Second chunk: length and data
        parser.read_buf(b"\r\nhello");
        let result = parser.try_parse();
        assert_eq!(result, Err(ParseError::NotEnoughData));

        // Third chunk: terminator
        parser.read_buf(b"\r\n");
        let result = parser.try_parse();
        assert_eq!(
            result,
            Ok(Some(RespValue::BulkString(Some(Cow::Borrowed("hello")))))
        );
    }

    // simple array chunked transfer
    {
        let mut parser = Parser::new(100, 1000);

        // First chunk: array length
        parser.read_buf(b"*2");
        _ = parser.try_parse();

        // Second chunk: array length terminator and first element start
        parser.read_buf(b"\r\n:1");
        _ = parser.try_parse();

        // Third chunk: first element terminator
        parser.read_buf(b"\r\n");
        _ = parser.try_parse();

        // Fourth chunk: second element
        parser.read_buf(b":2\r\n");
        let result = parser.try_parse();
        assert_eq!(
            result,
            Ok(Some(RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(2)
            ]))))
        );
    }
}
