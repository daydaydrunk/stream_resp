# stream_resp

StreamRESP is a RESP (Redis Serialization Protocol) parser implemented using a finite state machine (FSM) approach. Designed for streaming scenarios.

## Installation

To use `stream_resp` in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
stream_resp = "0.1"
```

## Usage

Here are some examples demonstrating how to use the `stream_resp` parser.

### Example 1: Parsing Complete RESP Messages

```rust
use std::borrow::Cow;
use stream_resp::parser::{ParseError, Parser};
use stream_resp::resp::RespValue;

fn main() {
    let mut parser = Parser::new(100, 1000);

    parser.read_buf(b"+OK\r\n");
    let result = match parser.try_parse() {
        Ok(Some(val)) => val,
        Ok(None) => panic!("Expected complete value"),
        Err(e) => panic!("Parse error: {:?}", e),
    };
    assert_eq!(result, RespValue::SimpleString(Cow::Borrowed("OK")));

    parser.read_buf(b"+Hello World\r\n");
    let result = match parser.try_parse() {
        Ok(Some(val)) => val,
        Ok(None) => panic!("Expected complete value"),
        Err(e) => panic!("Parse error: {:?}", e),
    };
    assert_eq!(
        result,
        RespValue::SimpleString(Cow::Borrowed("Hello World"))
    );
}
```

### Example 2: Parsing Incomplete RESP Messages in Chunks

```rust
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
        parser.read_buf(b"\\r\\nhello");
        let result = parser.try_parse();
        assert_eq!(result, Err(ParseError::NotEnoughData));

        // Third chunk: terminator
        parser.read_buf(b"\\r\\n");
        let result = parser.try_parse();
        assert_eq!(
            result,
            Ok(Some(RespValue::BulkString(Some(Cow::Borrowed("hello")))))
        );
    }

    // Simple array chunked transfer
    {
        let mut parser = Parser::new(100, 1000);

        // First chunk: array length
        parser.read_buf(b"*2");
        _ = parser.try_parse();

        // Second chunk: array length terminator and first element start
        parser.read_buf(b"\\r\\n:1");
        _ = parser.try_parse();

        // Third chunk: first element terminator
        parser.read_buf(b"\\r\\n");
        _ = parser.try_parse();

        // Fourth chunk: second element
        parser.read_buf(b":2\\r\\n");
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
```

## Contributing

Contributions are welcome! Please open an issue or submit a pull request on GitHub.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.