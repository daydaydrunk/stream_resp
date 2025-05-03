use std::borrow::Cow;
use stream_resp::parser::Parser;
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
    println!(
        "RESP Bytes: {:?}",
        String::from_utf8_lossy(&command.as_bytes())
    );

    // From String/&str (becomes SimpleString)
    let simple_str: RespValue = "OK".into();
    assert_eq!(simple_str, RespValue::SimpleString(Cow::Borrowed("OK")));

    let simple_string: RespValue = String::from("Hello").into();
    assert_eq!(
        simple_string,
        RespValue::SimpleString(Cow::Owned("Hello".to_string()))
    );

    // From i64
    let integer: RespValue = 123.into();
    assert_eq!(integer, RespValue::Integer(123));

    // From Option<String> (becomes BulkString)
    let bulk_some: RespValue = Some("data".to_string()).into();
    assert_eq!(
        bulk_some,
        RespValue::BulkString(Some(Cow::Owned("data".to_string())))
    );

    let bulk_none: RespValue = None::<String>.into();
    assert_eq!(bulk_none, RespValue::BulkString(None));

    // From Vec<RespValue> (becomes Array)
    let array: RespValue = vec![RespValue::Integer(1), "two".into()].into();
    assert_eq!(
        array,
        RespValue::Array(Some(vec![
            RespValue::Integer(1),
            RespValue::SimpleString(Cow::Borrowed("two"))
        ]))
    );

    let simple_string = RespValue::SimpleString(Cow::Borrowed("OK"));
    let ok_str: String = simple_string.into();
    assert_eq!(ok_str, "OK");

    let integer = RespValue::Integer(42);
    let num: i64 = integer.into(); // Panics if not Integer
    assert_eq!(num, 42);
}
