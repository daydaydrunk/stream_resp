# stream_resp

StreamRESP is a RESP (Redis Serialization Protocol) parser implemented using a finite state machine (FSM) approach. Designed for streaming scenarios.

## Installation

To use `stream_resp` in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
stream_resp = "0.1"
```
Enabling jemalloc
If you want to enable jemalloc for better memory allocation performance, you can enable the jemalloc feature in your Cargo.toml:
```toml
[dependencies]
stream_resp = { version = "0.1", features = ["jemalloc"] }
```

## Usage

Here are some examples demonstrating how to use the `stream_resp` parser.

### Example 1: Basic Usages
```rust
let value = RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2)]));
assert_eq!(value.as_bytes(), b"*2\r\n:1\r\n:2\r\n");
```

### Example 1: Streaming RESP Messages over TCP
```rust
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use stream_resp::parser::{ParseError, Parser};
use stream_resp::resp::RespValue;

fn handle_client(mut stream: TcpStream) {
    let mut parser = Parser::new(100, 1000);
    let mut buffer = [0; 512];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                parser.read_buf(&buffer[..n]);
                while let Ok(Some((resp, _))) = parser.try_parse() {
                    println!("Parsed RESP value: {:?}", resp);
                    // Echo the parsed RESP value back to the client
                    let response = format!("{:?}\r\n", resp);
                    stream.write_all(response.as_bytes()).unwrap();
                }
            }
            Err(e) => {
                eprintln!("Failed to read from socket: {:?}", e);
                break;
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379")?;
    println!("Server listening on port 6379");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
            }
        }
    }

    Ok(())
}
```

### Example 2: Parsing Complete RESP Messages

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

### Example 3: Parsing Incomplete RESP Messages in Chunks

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