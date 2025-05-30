# stream_resp

StreamRESP is a RESP (Redis Serialization Protocol) parser **fully compliant with RESP3**, implemented using a finite state machine (FSM) approach. Designed for streaming scenarios.

- **Full RESP3 support:** All RESP3 types are supported.
- **Optional explicit positive integer sign:** Enable the `explicit-positive-sign` feature to support parsing integers with an explicit `+` sign (e.g., `:+123\r\n`).

## Documentation
 [DeepWiki](https://deepwiki.com/daydaydrunk/stream_resp)

## Installation

To use `stream_resp` in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
stream_resp = "1"
```

## Features

### Enabling jemalloc
If you want to enable jemalloc for better memory allocation performance, you can enable the `jemalloc` feature in your `Cargo.toml`:
```toml
[dependencies]
stream_resp = { version = "1", features = ["jemalloc"] }
```

### Enabling Explicit Positive Integer Sign (`+`)
By default, the parser strictly adheres to the RESP specification for integers (`:<number>\r\n`). However, some clients or protocols might use an explicit plus sign (`:+<number>\r\n`). To enable parsing of integers with an explicit `+`, enable the `integer-plus-sign` feature:
```toml
[dependencies]
stream_resp = { version = "1", features = ["explicit-positive-sign"] }
```
You can also enable multiple features:
```toml
[dependencies]
stream_resp = { version = "1", features = ["jemalloc", "explicit-positive-sign"] }
```

## Usage

Here are some examples demonstrating how to use the `stream_resp` parser.

### Example 1: Basic Usages

#### Creating RESP Values using `From`

```rust
use std::borrow::Cow;
use stream_resp::resp::RespValue;

// From String/&str (becomes SimpleString)
let simple_str: RespValue = "OK".into();
assert_eq!(simple_str, RespValue::SimpleString(Cow::Borrowed("OK")));

let simple_string: RespValue = String::from("Hello").into();
assert_eq!(simple_string, RespValue::SimpleString(Cow::Owned("Hello".to_string())));

// From i64
let integer: RespValue = 123.into();
assert_eq!(integer, RespValue::Integer(123));

// From Option<String> (becomes BulkString)
let bulk_some: RespValue = Some("data".to_string()).into();
assert_eq!(bulk_some, RespValue::BulkString(Some(Cow::Owned("data".to_string()))));

let bulk_none: RespValue = None::<String>.into();
assert_eq!(bulk_none, RespValue::BulkString(None));

// From Vec<RespValue> (becomes Array)
let array: RespValue = vec![RespValue::Integer(1), "two".into()].into();
assert_eq!(array, RespValue::Array(Some(vec![
    RespValue::Integer(1),
    RespValue::SimpleString(Cow::Borrowed("two"))
])));

// Other From implementations exist for bool, f64, Vec<(RespValue, RespValue)> (Map), etc.
```

#### Representing Redis Commands

```rust
use std::borrow::Cow;
use stream_resp::resp::RespValue;

// Representing the Redis command: SET mykey "Hello"
let command = RespValue::Array(Some(vec![
    RespValue::BulkString(Some(Cow::Borrowed("SET"))),
    RespValue::BulkString(Some(Cow::Borrowed("mykey"))),
    RespValue::BulkString(Some(Cow::Borrowed("Hello"))),
]));

// Get the RESP byte representation
let expected_bytes = b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$5\r\nHello\r\n";
assert_eq!(command.as_bytes(), expected_bytes);

println!("Command: {:?}", command);
println!("RESP Bytes: {:?}", String::from_utf8_lossy(&command.as_bytes()));
```

#### Converting RESP Values using `Into` 

```rust
use std::borrow::Cow;
use stream_resp::resp::RespValue;

let simple_string = RespValue::SimpleString(Cow::Borrowed("OK"));
let ok_str: String = simple_string.into(); 
assert_eq!(ok_str, "OK");

let integer = RespValue::Integer(42);
let num: i64 = integer.into(); 
assert_eq!(num, 42);

```

### Example 2: Streaming RESP Messages over TCP
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

### Example 3: Parsing Complete RESP Messages

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

### Example 4: Parsing Incomplete RESP Messages in Chunks

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

## Benchmarks

The project includes performance benchmarks for the RESP parser. To run the benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark group
cargo bench --bench parser_benchmark -- "RESP Parser"

# Run a specific benchmark test
cargo bench --bench parser_benchmark -- "RESP Parser/parse/simple_string"
```

The benchmark results will be displayed in your terminal. Detailed HTML reports will be generated in the `target/criterion` directory, which you can open in a web browser to see more detailed performance information.

## Viewing Benchmark Results

After running the benchmarks, you can find detailed reports in:
```
target/criterion/report/index.html
```

The benchmarks measure parsing performance across various RESP data types and scenarios including:
- Simple strings, errors, integers
- Bulk strings (empty, null, large)
- Arrays (simple, nested, large)
- Real-world commands
- Batched commands
- Chunked parsing

Benchmark results will be displayed in the terminal and detailed HTML reports are generated in the 
`target/criterion` directory.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request on GitHub.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.