use crate::parser::{ParseError, Parser};
use crate::resp::RespValue;
use std::borrow::Cow;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn set_logger() {
        // Set up a subscriber with the desired log level
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .finish();

        // Initialize the global subscriber
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    }

    #[test]
    fn test_with_debug_log() {
        //set_logger();
        let mut parser = Parser::new(100, 1000);

        parser.read_buf(b"+simple string\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(
            result,
            RespValue::SimpleString(Cow::Borrowed("simple string"))
        );
    }

    #[test]
    fn test_simple_string() {
        //set_logger();
        let mut parser = Parser::new(100, 1000);

        // Basic case
        parser.read_buf(b"+OK\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::SimpleString(Cow::Borrowed("OK")));

        // Note: Simple String should not contain CR or LF
        // These should be transmitted using Bulk String
        parser.read_buf(b"+Hello World\r\n"); // Correct
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(
            result,
            RespValue::SimpleString(Cow::Borrowed("Hello World"))
        );

        // Test other valid special characters
        parser.read_buf(b"+Hello@#$%^&*()\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(
            result,
            RespValue::SimpleString(Cow::Borrowed("Hello@#$%^&*()"))
        );
    }

    #[test]
    fn test_error() {
        let mut parser = Parser::new(100, 1000);

        // Basic error
        parser.read_buf(b"-Error message\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Error(Cow::Borrowed("Error message")));

        // Empty error
        parser.read_buf(b"-\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Error(Cow::Borrowed("")));

        // Redis style error
        parser.read_buf(b"-ERR unknown command 'foobar'\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(
            result,
            RespValue::Error(Cow::Borrowed("ERR unknown command 'foobar'"))
        );
    }

    #[test]
    fn test_integer() {
        let mut parser = Parser::new(100, 1000);

        // Positive number
        parser.read_buf(b":1234\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Integer(1234));

        // Negative number
        parser.read_buf(b":-1234\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Integer(-1234));

        // Zero
        parser.read_buf(b":0\r\n");
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Integer(0));

        // Maximum value
        parser.read_buf(format!(":{}\r\n", i64::MAX).as_bytes());
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Integer(i64::MAX));

        // Minimum value
        parser.read_buf(format!(":{}\r\n", i64::MIN).as_bytes());
        let result = match parser.try_parse() {
            Ok(Some(val)) => val,
            Ok(None) => panic!("Expected complete value"),
            Err(e) => panic!("Parse error: {:?}", e),
        };
        assert_eq!(result, RespValue::Integer(i64::MIN));
    }

    #[test]
    fn test_invalid_type_marker() {
        let mut parser = Parser::new(100, 1000);
        parser.read_buf(b"x1234");
        match parser.try_parse() {
            Err(ParseError::InvalidFormat(_)) => (), // Expected error
            other => panic!("Expected InvalidFormat error, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_length() {
        let mut parser = Parser::new(100, 1000);
        parser.read_buf(b"$-2");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!("Expected None for incomplete data, got {:?}", other),
        }

        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::Null)) => (), // Expected error
            other => panic!("Expected InvalidLength error, got {:?}", other),
        }
    }

    #[test]
    fn test_array_length_mismatch() {
        let mut parser = Parser::new(100, 1000);
        parser.read_buf(b"*2\r\n+OK\r\n");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected incomplete state
            other => panic!("Expected None for incomplete array, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_integer_format() {
        let mut parser = Parser::new(100, 1000);
        parser.read_buf(b":12.34");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!("Expected None for incomplete data, got {:?}", other),
        }

        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Err(ParseError::InvalidFormat(_)) => (), // Expected error
            other => panic!("Expected InvalidFormat error, got {:?}", other),
        }
    }

    #[test]
    fn test_missing_crlf() {
        let mut parser = Parser::new(100, 1000);
        parser.read_buf(b"+OK\n");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected error
            other => panic!("Expected InvalidFormat error, got {:?}", other),
        }
    }

    #[test]
    fn test_exceeding_maximum_depth() {
        let mut shallow_parser = Parser::new(1, 1000);
        shallow_parser.read_buf(b"*1\r\n");
        match shallow_parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!("Expected None for incomplete data, got {:?}", other),
        }

        shallow_parser.read_buf(b"*1\r\n");
        match shallow_parser.try_parse() {
            Err(ParseError::InvalidDepth) => (), // Waiting for more data
            other => panic!("Expected None for incomplete data, got {:?}", other),
        }

        shallow_parser.read_buf(b"+OK\r\n");
        match shallow_parser.try_parse() {
            Err(ParseError::InvalidDepth) => (), // Expected error
            other => panic!(
                "Expected InvalidFormat error for exceeding maximum depth, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_incomplete_messages() {
        let mut parser = Parser::new(100, 1000);

        // Incomplete simple string
        parser.read_buf(b"+OK");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!(
                "Expected None for incomplete simple string, got {:?}",
                other
            ),
        }

        // Reset parser
        parser = Parser::new(100, 1000);

        // Incomplete error message
        parser.read_buf(b"-ERR");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!(
                "Expected None for incomplete error message, got {:?}",
                other
            ),
        }

        // Reset parser
        parser = Parser::new(100, 1000);

        // Incomplete integer
        parser.read_buf(b":123");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!("Expected None for incomplete integer, got {:?}", other),
        }

        // Reset parser
        parser = Parser::new(100, 1000);

        // Incomplete bulk string length
        parser.read_buf(b"$5");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!(
                "Expected None for incomplete bulk string length, got {:?}",
                other
            ),
        }

        // Reset parser
        parser = Parser::new(100, 1000);

        // Incomplete array length
        parser.read_buf(b"*3");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Waiting for more data
            other => panic!("Expected None for incomplete array length, got {:?}", other),
        }
    }

    #[test]
    fn test_large_messages() {
        let mut parser = Parser::new(100, 10000);

        // Large string
        let large_string = "x".repeat(1000);
        let _message = format!("${}\r\n{}\r\n", large_string.len(), large_string);

        // Send length information in chunks
        parser.read_buf(format!("${}\r\n", large_string.len()).as_bytes());
        match parser.try_parse() {
            Err(ParseError::NotEnoughData) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Send data in chunks
        let chunks = large_string.as_bytes().chunks(100);
        for chunk in chunks {
            parser.read_buf(chunk);
            match parser.try_parse() {
                Err(ParseError::NotEnoughData) => (), // Expected to wait for more data
                other => panic!("Expected None, got {:?}", other),
            }
        }

        // Send terminator
        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::BulkString(Some(msg)))) => {
                assert_eq!(msg, large_string);
            }
            other => panic!("Expected BulkString, got {:?}", other),
        }

        // Large array
        let large_array = String::from("*1000\r\n");
        parser.read_buf(large_array.as_bytes());
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Send array elements in chunks
        for _ in 0..999 {
            parser.read_buf(b":1\r\n");
            match parser.try_parse() {
                Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
                other => panic!("Expected None, got {:?}", other),
            }
        }

        // Send last element
        parser.read_buf(b":1\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::Array(Some(arr)))) => {
                assert_eq!(arr.len(), 1000);
                assert!(arr.iter().all(|x| *x == RespValue::Integer(1)));
            }
            other => panic!("Expected Array, got {:?}", other),
        }
    }

    #[test]
    fn test_error_message_chunks() {
        let mut parser = Parser::new(100, 1000);

        // First chunk: only error type marker and part of the message
        parser.read_buf(b"-ERR unknow");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Second chunk: continue adding message
        parser.read_buf(b"n command");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Third chunk: add terminator
        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::Error(msg))) => {
                assert_eq!(msg, "ERR unknown command");
            }
            other => panic!("Expected Error message, got {:?}", other),
        }
    }

    #[test]
    fn test_bulk_string_chunks() {
        // Test complete input
        {
            let mut parser = Parser::new(100, 1000);
            parser.read_buf(b"$0\r\n\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Ok(Some(RespValue::Null)));
        }

        // Test two chunks
        {
            let mut parser = Parser::new(100, 1000);

            // First chunk: type marker and length
            parser.read_buf(b"$0\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Ok(Some(RespValue::Null))); // Still need more data

            // Second chunk: terminator
            parser.read_buf(b"\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::UnexpectedEof));
        }

        // Test three chunks
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

        // Test non-empty string chunked transfer
        {
            let mut parser = Parser::new(100, 1000);

            // First chunk: header
            parser.read_buf(b"$12\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::NotEnoughData));

            // Second chunk: partial data
            parser.read_buf(b"Hello ");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::NotEnoughData));

            // Third chunk: remaining data
            parser.read_buf(b"World!");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::NotEnoughData));

            // Fourth chunk: terminator
            parser.read_buf(b"\r\n");
            let result = parser.try_parse();
            assert_eq!(
                result,
                Ok(Some(RespValue::BulkString(Some(Cow::Borrowed(
                    "Hello World!"
                )))))
            );
        }
    }

    #[test]
    fn test_array_chunks() {
        // Test simple array chunked transfer
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

        // Test empty array
        {
            let mut parser = Parser::new(100, 1000);
            parser.read_buf(b"*0\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Ok(Some(RespValue::Array(None))));
        }

        // Test null array
        {
            let mut parser = Parser::new(100, 1000);
            parser.read_buf(b"*-1\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Ok(Some(RespValue::Array(None))));
        }

        // Test mixed type array
        {
            let mut parser = Parser::new(100, 1000);

            // Send array header and first element (integer)
            parser.read_buf(b"*3\r\n:123\r\n");
            _ = parser.try_parse(); // Need more elements

            // Send second element (simple string)
            parser.read_buf(b"+hello\r\n");
            _ = parser.try_parse(); // Need more elements

            // Send third element (bulk string)
            parser.read_buf(b"$5\r\nworld\r\n");
            let result = parser.try_parse();
            assert_eq!(
                result,
                Ok(Some(RespValue::Array(Some(vec![
                    RespValue::Integer(123),
                    RespValue::SimpleString("hello".into()),
                    RespValue::BulkString(Some("world".into()))
                ]))))
            );
        }

        // Test nested array
        {
            let mut parser = Parser::new(100, 1000);

            // Outer array start
            parser.read_buf(b"*2\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::UnexpectedEof));

            // Inner array 1
            parser.read_buf(b"*2\r\n+a\r\n+b\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::UnexpectedEof));

            // Inner array 2
            parser.read_buf(b"*2\r\n+c\r\n+d\r\n");
            let result = parser.try_parse();
            assert_eq!(
                result,
                Ok(Some(RespValue::Array(Some(vec![
                    RespValue::Array(Some(vec![
                        RespValue::SimpleString(Cow::Borrowed("a")),
                        RespValue::SimpleString(Cow::Borrowed("b"))
                    ])),
                    RespValue::Array(Some(vec![
                        RespValue::SimpleString(Cow::Borrowed("c")),
                        RespValue::SimpleString(Cow::Borrowed("d"))
                    ]))
                ]))))
            );
        }

        // Test large array chunked transfer
        // {
        //     let mut parser = Parser::new(100, 1000);

        //     // Send array length
        //     parser.read_buf(b"*3\r\n");
        //     let result = parser.try_parse();
        //     assert_eq!(result, Err(ParseError::UnexpectedEof));

        //     // Send many integers one by one
        //     for i in 1..=3 {
        //         parser.read_buf(format!(":1{}\r\n", i).as_bytes());
        //         let expected = if i < 3 {
        //             Ok(None)
        //         } else {
        //             Ok(Some(RespValue::Array(Some(vec![
        //                 RespValue::Integer(11),
        //                 RespValue::Integer(12),
        //                 RespValue::Integer(13),
        //             ]))))
        //         };
        //         assert_eq!(parser.try_parse(), expected);
        //     }
        // }

        // Test error cases
        {
            let mut parser = Parser::new(100, 1000);

            // Invalid array length
            parser.read_buf(b"*-2\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Ok(Some(RespValue::Array(None))));

            // Reset parser
            parser = Parser::new(100, 1000);

            // Incomplete array elements
            parser.read_buf(b"*2\r\n:1\r\n");
            let result = parser.try_parse();
            assert_eq!(result, Err(ParseError::UnexpectedEof)); // Need more elements
        }

        let mut parser = Parser::new(100, 1000);

        // Empty array
        parser.read_buf(b"*0\r\n");
        assert_eq!(parser.try_parse(), Ok(Some(RespValue::Array(None))));

        // Null array
        parser.read_buf(b"*-1\r\n");
        assert_eq!(parser.try_parse(), Ok(Some(RespValue::Array(None))));

        // Simple array
        parser.read_buf(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");
        assert_eq!(
            parser.try_parse(),
            Ok(Some(RespValue::Array(Some(vec![
                RespValue::BulkString(Some(Cow::Borrowed("hello"))),
                RespValue::BulkString(Some(Cow::Borrowed("world")))
            ]))))
        );

        // Mixed type array
        parser.read_buf(b"*5\r\n:1\r\n:2\r\n:3\r\n:4\r\n$5\r\nhello\r\n");
        assert_eq!(
            parser.try_parse(),
            Ok(Some(RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(2),
                RespValue::Integer(3),
                RespValue::Integer(4),
                RespValue::BulkString(Some(Cow::Borrowed("hello")))
            ]))))
        );

        // Nested array
        parser.read_buf(b"*2\r\n*2\r\n+a\r\n+b\r\n*2\r\n+c\r\n+d\r\n");
        assert_eq!(
            parser.try_parse(),
            Ok(Some(RespValue::Array(Some(vec![
                RespValue::Array(Some(vec![
                    RespValue::SimpleString(Cow::Borrowed("a")),
                    RespValue::SimpleString(Cow::Borrowed("b"))
                ])),
                RespValue::Array(Some(vec![
                    RespValue::SimpleString(Cow::Borrowed("c")),
                    RespValue::SimpleString(Cow::Borrowed("d"))
                ]))
            ]))))
        );
    }

    #[test]
    fn test_integer_chunks() {
        let mut parser = Parser::new(100, 1000);

        // First chunk: type marker and partial number
        parser.read_buf(b":123");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Second chunk: remaining number
        parser.read_buf(b"45");
        match parser.try_parse() {
            Err(ParseError::UnexpectedEof) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Third chunk: terminator
        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::Integer(num))) => {
                assert_eq!(num, 12345);
            }
            other => panic!("Expected Integer, got {:?}", other),
        }
    }

    #[test]
    fn test_large_bulk_string_chunks() {
        let mut parser = Parser::new(100, 10000);

        // Construct large string
        let large_string = "x".repeat(1000);

        // First chunk: length prefix
        parser.read_buf(format!("${}\r\n", large_string.len()).as_bytes());
        match parser.try_parse() {
            Err(ParseError::NotEnoughData) => (), // Expected to wait for more data
            other => panic!("Expected None, got {:?}", other),
        }

        // Send large string in multiple chunks
        let chunk_size = 100;
        for chunk in large_string.as_bytes().chunks(chunk_size) {
            parser.read_buf(chunk);
            match parser.try_parse() {
                Err(ParseError::NotEnoughData) => (), // Expected to wait for more data
                other => panic!("Expected None while processing chunks, got {:?}", other),
            }
        }

        // Finally send terminator
        parser.read_buf(b"\r\n");
        match parser.try_parse() {
            Ok(Some(RespValue::BulkString(Some(msg)))) => {
                assert_eq!(msg, large_string);
            }
            other => panic!("Expected BulkString, got {:?}", other),
        }
    }
}
