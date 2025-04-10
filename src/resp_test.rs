#[allow(dead_code)]
use crate::resp::RespValue;
use std::borrow::Cow;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_none() {
        assert!(!RespValue::SimpleString(Cow::Borrowed("test")).is_none());
        assert!(!RespValue::SimpleString(Cow::Borrowed("")).is_none());
        assert!(!RespValue::Error(Cow::Borrowed("error")).is_none());
        assert!(!RespValue::Error(Cow::Borrowed("")).is_none());
        assert!(!RespValue::Integer(0).is_none());

        assert!(RespValue::BulkString(None).is_none());
        assert!(RespValue::BulkString(Some(Cow::Borrowed(""))).is_none());
        assert!(!RespValue::BulkString(Some(Cow::Borrowed("test"))).is_none());

        assert!(RespValue::Array(None).is_none());
        assert!(RespValue::Array(Some(vec![])).is_none());
        assert!(!RespValue::Array(Some(vec![RespValue::Integer(1)])).is_none());

        assert!(RespValue::Map(None).is_none());
        assert!(RespValue::Map(Some(vec![])).is_none());
        assert!(!RespValue::Map(Some(vec![(
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::SimpleString(Cow::Borrowed("value"))
        )]))
        .is_none());

        assert!(RespValue::Set(None).is_none());
        assert!(RespValue::Set(Some(vec![])).is_none());
        assert!(!RespValue::Set(Some(vec![RespValue::Integer(1)])).is_none());

        assert!(RespValue::Null.is_none());

        assert!(!RespValue::Boolean(true).is_none());
        assert!(!RespValue::Double(1.23).is_none());
        assert!(!RespValue::BigNumber(Cow::Borrowed("12345")).is_none());

        assert!(!RespValue::VerbatimString(Some(Cow::Borrowed("hello"))).is_none());

        assert!(RespValue::Push(None).is_none());
        assert!(!RespValue::Push(Some(vec![RespValue::Integer(1)])).is_none());

        assert!(RespValue::Map(None).is_none());
        assert!(!RespValue::Map(Some(vec![(
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::SimpleString(Cow::Borrowed("value"))
        )]))
        .is_none());
    }

    #[test]
    fn test_simple_string() {
        let value = RespValue::SimpleString(Cow::Borrowed("OK"));
        assert_eq!(value.as_bytes(), b"+OK\r\n");

        let value = RespValue::SimpleString(Cow::Borrowed(""));
        assert_eq!(value.as_bytes(), b"+\r\n");

        let value = RespValue::SimpleString(Cow::Borrowed("Hello World"));
        assert_eq!(value.as_bytes(), b"+Hello World\r\n");
    }

    #[test]
    fn test_error() {
        let value = RespValue::Error(Cow::Borrowed("Error message"));
        assert_eq!(value.as_bytes(), b"-Error message\r\n");

        let value = RespValue::Error(Cow::Borrowed(""));
        assert_eq!(value.as_bytes(), b"-\r\n");

        let value = RespValue::Error(Cow::Borrowed("ERR unknown command"));
        assert_eq!(value.as_bytes(), b"-ERR unknown command\r\n");
    }

    #[test]
    fn test_integer() {
        let value = RespValue::Integer(0);
        assert_eq!(value.as_bytes(), b":0\r\n");

        let value = RespValue::Integer(-1);
        assert_eq!(value.as_bytes(), b":-1\r\n");

        let value = RespValue::Integer(1000);
        assert_eq!(value.as_bytes(), b":1000\r\n");

        let value = RespValue::Integer(i64::MAX);
        assert_eq!(value.as_bytes(), format!(":{}\r\n", i64::MAX).as_bytes());

        let value = RespValue::Integer(i64::MIN);
        assert_eq!(value.as_bytes(), format!(":{}\r\n", i64::MIN).as_bytes());
    }

    #[test]
    fn test_bulk_string() {
        let value = RespValue::BulkString(Some(Cow::Borrowed("hello")));
        assert_eq!(value.as_bytes(), b"$5\r\nhello\r\n");

        let value = RespValue::BulkString(Some(Cow::Borrowed("")));
        assert_eq!(value.as_bytes(), b"$0\r\n\r\n");

        let value = RespValue::BulkString(None);
        assert_eq!(value.as_bytes(), b"$-1\r\n");

        let long_string = "a".repeat(1000);
        let value = RespValue::BulkString(Some(Cow::Owned(long_string.clone())));
        assert_eq!(
            value.as_bytes(),
            format!("$1000\r\n{}\r\n", long_string).as_bytes()
        );
    }

    #[test]
    fn test_array() {
        let value = RespValue::Array(Some(vec![]));
        assert_eq!(value.as_bytes(), b"*0\r\n");

        let value = RespValue::Array(None);
        assert_eq!(value.as_bytes(), b"*-1\r\n");

        let value = RespValue::Array(Some(vec![
            RespValue::SimpleString(Cow::Borrowed("OK")),
            RespValue::Integer(123),
            RespValue::BulkString(Some(Cow::Borrowed("hello"))),
        ]));
        assert_eq!(value.as_bytes(), b"*3\r\n+OK\r\n:123\r\n$5\r\nhello\r\n");

        let value = RespValue::Array(Some(vec![
            RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2)])),
            RespValue::Array(Some(vec![RespValue::Integer(3), RespValue::Integer(4)])),
        ]));
        assert_eq!(
            value.as_bytes(),
            b"*2\r\n*2\r\n:1\r\n:2\r\n*2\r\n:3\r\n:4\r\n"
        );
    }

    #[test]
    fn test_null() {
        let value = RespValue::Null;
        assert_eq!(value.as_bytes(), b"_\r\n");
    }

    #[test]
    fn test_boolean() {
        let value = RespValue::Boolean(true);
        assert_eq!(value.as_bytes(), b"#t\r\n");

        let value = RespValue::Boolean(false);
        assert_eq!(value.as_bytes(), b"#f\r\n");
    }

    #[test]
    fn test_double() {
        let value = RespValue::Double(3.14);
        assert_eq!(value.as_bytes(), b",3.14\r\n");

        let value = RespValue::Double(-0.5);
        assert_eq!(value.as_bytes(), b",-0.5\r\n");

        let value = RespValue::Double(0.0);
        assert_eq!(value.as_bytes(), b",0\r\n");
    }

    #[test]
    fn test_big_number() {
        let value =
            RespValue::BigNumber(Cow::Borrowed("3492890328409238509324850943850943825024385"));
        assert_eq!(
            value.as_bytes(),
            b"(3492890328409238509324850943850943825024385\r\n"
        );

        let value = RespValue::BigNumber(Cow::Borrowed(
            "-3492890328409238509324850943850943825024385",
        ));
        assert_eq!(
            value.as_bytes(),
            b"(-3492890328409238509324850943850943825024385\r\n"
        );
    }

    #[test]
    fn test_bulk_error() {
        let value = RespValue::BulkError(Some(Cow::Borrowed("Error details")));
        assert_eq!(value.as_bytes(), b"!Error details\r\n");

        let value = RespValue::BulkError(None);
        assert_eq!(value.as_bytes(), b"!-1\r\n");
    }

    #[test]
    fn test_verbatim_string() {
        let value = RespValue::VerbatimString(Some(Cow::Borrowed("txt:Some text")));
        assert_eq!(value.as_bytes(), b"=txt:Some text\r\n");

        let value = RespValue::VerbatimString(None);
        assert_eq!(value.as_bytes(), b"=-1\r\n");
    }

    #[test]
    fn test_map() {
        let value = RespValue::Map(Some(vec![]));
        assert_eq!(value.as_bytes(), b"%0\r\n");

        let value = RespValue::Map(None);
        assert_eq!(value.as_bytes(), b"%-1\r\n");

        let value = RespValue::Map(Some(vec![
            (
                RespValue::SimpleString(Cow::Borrowed("key1")),
                RespValue::Integer(123),
            ),
            (
                RespValue::SimpleString(Cow::Borrowed("key2")),
                RespValue::BulkString(Some(Cow::Borrowed("value"))),
            ),
        ]));
        assert_eq!(
            value.as_bytes(),
            b"%2\r\n+key1\r\n:123\r\n+key2\r\n$5\r\nvalue\r\n"
        );
    }

    #[test]
    fn test_set() {
        let value = RespValue::Set(Some(vec![]));
        assert_eq!(value.as_bytes(), b"~0\r\n");

        let value = RespValue::Set(None);
        assert_eq!(value.as_bytes(), b"~-1\r\n");

        let value = RespValue::Set(Some(vec![
            RespValue::Integer(1),
            RespValue::SimpleString(Cow::Borrowed("two")),
            RespValue::BulkString(Some(Cow::Borrowed("three"))),
        ]));
        assert_eq!(value.as_bytes(), b"~3\r\n:1\r\n+two\r\n$5\r\nthree\r\n");
    }

    #[test]
    fn test_push() {
        let value = RespValue::Push(Some(vec![]));
        assert_eq!(value.as_bytes(), b">0\r\n");

        let value = RespValue::Push(None);
        assert_eq!(value.as_bytes(), b">-1\r\n");

        let value = RespValue::Push(Some(vec![
            RespValue::SimpleString(Cow::Borrowed("message")),
            RespValue::Integer(42),
        ]));
        assert_eq!(value.as_bytes(), b">2\r\n+message\r\n:42\r\n");
    }

    #[test]
    fn test_into_owned() {
        let borrowed = RespValue::SimpleString(Cow::Borrowed("test"));
        let owned = borrowed.into_owned();
        match owned {
            RespValue::SimpleString(s) => {
                assert!(matches!(s, Cow::Owned(_)));
                assert_eq!(s, "test");
            }
            _ => panic!("Wrong variant"),
        }

        let borrowed = RespValue::Array(Some(vec![
            RespValue::SimpleString(Cow::Borrowed("test")),
            RespValue::BulkString(Some(Cow::Borrowed("bulk"))),
        ]));
        let owned = borrowed.into_owned();
        match owned {
            RespValue::Array(Some(arr)) => {
                assert_eq!(arr.len(), 2);
                match &arr[0] {
                    RespValue::SimpleString(s) => {
                        assert!(matches!(s, Cow::Owned(_)));
                        assert_eq!(s, "test");
                    }
                    _ => panic!("Wrong variant"),
                }
                match &arr[1] {
                    RespValue::BulkString(Some(s)) => {
                        assert!(matches!(s, Cow::Owned(_)));
                        assert_eq!(s, "bulk");
                    }
                    _ => panic!("Wrong variant"),
                }
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_complex_nested_structures() {
        let value = RespValue::Array(Some(vec![
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key1")),
                RespValue::Set(Some(vec![RespValue::Integer(1), RespValue::Integer(2)])),
            )])),
            RespValue::Push(Some(vec![
                RespValue::BulkString(Some(Cow::Borrowed("notification"))),
                RespValue::Array(Some(vec![
                    RespValue::SimpleString(Cow::Borrowed("data1")),
                    RespValue::SimpleString(Cow::Borrowed("data2")),
                ])),
            ])),
        ]));

        let bytes = value.as_bytes();
        assert!(bytes.starts_with(b"*2\r\n"));
        assert!(bytes.len() > 20);
    }

    #[test]
    fn test_resp_value_size() {
        println!("RespValue size: {}", std::mem::size_of::<RespValue>());
        println!("RespValue alignment: {}", std::mem::align_of::<RespValue>());

        // Ensure no unexpected padding
        assert!(std::mem::size_of::<RespValue>() % 8 == 0);
    }

    #[test]
    fn test_from_string() {
        let value: RespValue = "test".to_string().into();
        assert_eq!(
            value,
            RespValue::SimpleString(Cow::Owned("test".to_string()))
        );
    }

    #[test]
    fn test_from_str() {
        let value: RespValue = "test".into();
        assert_eq!(value, RespValue::SimpleString(Cow::Borrowed("test")));
    }

    #[test]
    fn test_from_i64() {
        let value: RespValue = 42.into();
        assert_eq!(value, RespValue::Integer(42));
    }

    #[test]
    fn test_from_option_string() {
        let value: RespValue = Some("test".to_string()).into();
        assert_eq!(
            value,
            RespValue::BulkString(Some(Cow::Owned("test".to_string())))
        );

        let value: RespValue = None.into();
        assert_eq!(value, RespValue::BulkString(None));
    }

    #[test]
    fn test_from_vec_resp_value() {
        let value: RespValue = vec![RespValue::Integer(1), RespValue::Integer(2)].into();
        assert_eq!(
            value,
            RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2)]))
        );
    }

    #[test]
    fn test_from_bool() {
        let value: RespValue = true.into();
        assert_eq!(value, RespValue::Boolean(true));
    }

    #[test]
    fn test_from_f64() {
        let value: RespValue = 3.14.into();
        assert_eq!(value, RespValue::Double(3.14));
    }

    #[test]
    fn test_from_tuple_resp_value() {
        let value: RespValue = (
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::Integer(42),
        )
            .into();
        assert_eq!(
            value,
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )]))
        );
    }

    #[test]
    fn test_from_vec_tuple_resp_value() {
        let value: RespValue = vec![(
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::Integer(42),
        )]
        .into();
        assert_eq!(
            value,
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )]))
        );
    }

    #[test]
    fn test_into_string() {
        let value: String = RespValue::SimpleString(Cow::Owned("test".to_string())).into();
        assert_eq!(value, "test".to_string());
    }

    #[test]
    fn test_into_i64() {
        let value: i64 = RespValue::Integer(42).into();
        assert_eq!(value, 42);
    }

    #[test]
    fn test_into_option_string() {
        let value: Option<String> =
            RespValue::BulkString(Some(Cow::Owned("test".to_string()))).into();
        assert_eq!(value, Some("test".to_string()));

        let value: Option<String> = RespValue::BulkString(None).into();
        assert_eq!(value, None);
    }

    #[test]
    fn test_into_vec_resp_value() {
        let value: Vec<RespValue> =
            RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2)])).into();
        assert_eq!(value, vec![RespValue::Integer(1), RespValue::Integer(2)]);
    }

    #[test]
    fn test_into_bool() {
        let value: bool = RespValue::Boolean(true).into();
        assert_eq!(value, true);
    }

    #[test]
    fn test_into_f64() {
        let value: f64 = RespValue::Double(3.14).into();
        assert_eq!(value, 3.14);
    }

    #[test]
    fn test_into_vec_tuple_resp_value() {
        let value: Vec<(RespValue, RespValue)> = RespValue::Map(Some(vec![(
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::Integer(42),
        )]))
        .into();
        assert_eq!(
            value,
            vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )]
        );
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(
            RespValue::SimpleString(Cow::Borrowed("test")),
            RespValue::SimpleString(Cow::Borrowed("test"))
        );
        assert_ne!(
            RespValue::SimpleString(Cow::Borrowed("test")),
            RespValue::SimpleString(Cow::Borrowed("different"))
        );

        assert_eq!(
            RespValue::Error(Cow::Borrowed("error")),
            RespValue::Error(Cow::Borrowed("error"))
        );
        assert_ne!(
            RespValue::Error(Cow::Borrowed("error")),
            RespValue::Error(Cow::Borrowed("different"))
        );

        assert_eq!(RespValue::Integer(42), RespValue::Integer(42));
        assert_ne!(RespValue::Integer(42), RespValue::Integer(43));

        assert_eq!(
            RespValue::BulkString(Some(Cow::Borrowed("bulk"))),
            RespValue::BulkString(Some(Cow::Borrowed("bulk")))
        );
        assert_ne!(
            RespValue::BulkString(Some(Cow::Borrowed("bulk"))),
            RespValue::BulkString(Some(Cow::Borrowed("different")))
        );

        assert_eq!(
            RespValue::Array(Some(vec![RespValue::Integer(1)])),
            RespValue::Array(Some(vec![RespValue::Integer(1)]))
        );
        assert_ne!(
            RespValue::Array(Some(vec![RespValue::Integer(1)])),
            RespValue::Array(Some(vec![RespValue::Integer(2)]))
        );

        assert_eq!(RespValue::Null, RespValue::Null);

        assert_eq!(RespValue::Boolean(true), RespValue::Boolean(true));
        assert_ne!(RespValue::Boolean(true), RespValue::Boolean(false));

        assert_eq!(RespValue::Double(3.14), RespValue::Double(3.14));
        assert_ne!(RespValue::Double(3.14), RespValue::Double(2.71));

        assert_eq!(
            RespValue::BigNumber(Cow::Borrowed("12345")),
            RespValue::BigNumber(Cow::Borrowed("12345"))
        );
        assert_ne!(
            RespValue::BigNumber(Cow::Borrowed("12345")),
            RespValue::BigNumber(Cow::Borrowed("54321"))
        );

        assert_eq!(
            RespValue::BulkError(Some(Cow::Borrowed("error"))),
            RespValue::BulkError(Some(Cow::Borrowed("error")))
        );
        assert_ne!(
            RespValue::BulkError(Some(Cow::Borrowed("error"))),
            RespValue::BulkError(Some(Cow::Borrowed("different")))
        );

        assert_eq!(
            RespValue::VerbatimString(Some(Cow::Borrowed("verbatim"))),
            RespValue::VerbatimString(Some(Cow::Borrowed("verbatim")))
        );
        assert_ne!(
            RespValue::VerbatimString(Some(Cow::Borrowed("verbatim"))),
            RespValue::VerbatimString(Some(Cow::Borrowed("different")))
        );

        assert_eq!(
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )])),
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )]))
        );
        assert_ne!(
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(42)
            )])),
            RespValue::Map(Some(vec![(
                RespValue::SimpleString(Cow::Borrowed("key")),
                RespValue::Integer(43)
            )]))
        );

        assert_eq!(
            RespValue::Set(Some(vec![RespValue::Integer(1)])),
            RespValue::Set(Some(vec![RespValue::Integer(1)]))
        );
        assert_ne!(
            RespValue::Set(Some(vec![RespValue::Integer(1)])),
            RespValue::Set(Some(vec![RespValue::Integer(2)]))
        );

        assert_eq!(
            RespValue::Push(Some(vec![RespValue::Integer(1)])),
            RespValue::Push(Some(vec![RespValue::Integer(1)]))
        );
        assert_ne!(
            RespValue::Push(Some(vec![RespValue::Integer(1)])),
            RespValue::Push(Some(vec![RespValue::Integer(2)]))
        );
    }

    #[test]
    fn test_default() {
        let value: RespValue = Default::default();
        assert_eq!(value, RespValue::Null);
    }

    #[test]
    fn test_as_bytes() {
        let value = RespValue::SimpleString(Cow::Borrowed("OK"));
        assert_eq!(value.as_bytes(), b"+OK\r\n");

        let value = RespValue::Error(Cow::Borrowed("Error message"));
        assert_eq!(value.as_bytes(), b"-Error message\r\n");

        let value = RespValue::Integer(42);
        assert_eq!(value.as_bytes(), b":42\r\n");

        let value = RespValue::BulkString(Some(Cow::Borrowed("bulk")));
        assert_eq!(value.as_bytes(), b"$4\r\nbulk\r\n");

        let value = RespValue::Null;
        assert_eq!(value.as_bytes(), b"_\r\n");

        let value = RespValue::Array(Some(vec![RespValue::Integer(1), RespValue::Integer(2)]));
        assert_eq!(value.as_bytes(), b"*2\r\n:1\r\n:2\r\n");

        let value = RespValue::Boolean(true);
        assert_eq!(value.as_bytes(), b"#t\r\n");

        let value = RespValue::Double(3.14);
        assert_eq!(value.as_bytes(), b",3.14\r\n");

        let value = RespValue::BigNumber(Cow::Borrowed("12345"));
        assert_eq!(value.as_bytes(), b"(12345\r\n");

        let value = RespValue::BulkError(Some(Cow::Borrowed("error")));
        assert_eq!(value.as_bytes(), b"!error\r\n");

        let value = RespValue::VerbatimString(Some(Cow::Borrowed("verbatim")));
        assert_eq!(value.as_bytes(), b"=verbatim\r\n");

        let value = RespValue::Map(Some(vec![(
            RespValue::SimpleString(Cow::Borrowed("key")),
            RespValue::Integer(42),
        )]));
        assert_eq!(value.as_bytes(), b"%1\r\n+key\r\n:42\r\n");

        let value = RespValue::Set(Some(vec![RespValue::Integer(1), RespValue::Integer(2)]));
        assert_eq!(value.as_bytes(), b"~2\r\n:1\r\n:2\r\n");

        let value = RespValue::Push(Some(vec![RespValue::Integer(1), RespValue::Integer(2)]));
        assert_eq!(value.as_bytes(), b">2\r\n:1\r\n:2\r\n");
    }

    #[test]
    fn test_bulk_string_empty() {
        let value = RespValue::BulkString(Some(Cow::Borrowed("")));
        assert_eq!(value.as_bytes(), b"$0\r\n\r\n");
    }

    #[test]
    fn test_bulk_string_none() {
        let value = RespValue::BulkString(None);
        assert_eq!(value.as_bytes(), b"$-1\r\n");
    }

    #[test]
    fn test_bulk_error_empty() {
        let value = RespValue::BulkError(Some(Cow::Borrowed("")));
        assert_eq!(value.as_bytes(), b"!\r\n");
    }

    #[test]
    fn test_bulk_error_none() {
        let value = RespValue::BulkError(None);
        assert_eq!(value.as_bytes(), b"!-1\r\n");
    }

    #[test]
    fn test_verbatim_string_empty() {
        let value = RespValue::VerbatimString(Some(Cow::Borrowed("")));
        assert_eq!(value.as_bytes(), b"=\r\n");
    }

    #[test]
    fn test_verbatim_string_none() {
        let value = RespValue::VerbatimString(None);
        assert_eq!(value.as_bytes(), b"=-1\r\n");
    }

    #[test]
    fn test_map_empty() {
        let value = RespValue::Map(Some(vec![]));
        assert_eq!(value.as_bytes(), b"%0\r\n");
    }

    #[test]
    fn test_map_none() {
        let value = RespValue::Map(None);
        assert_eq!(value.as_bytes(), b"%-1\r\n");
    }

    #[test]
    fn test_set_empty() {
        let value = RespValue::Set(Some(vec![]));
        assert_eq!(value.as_bytes(), b"~0\r\n");
    }

    #[test]
    fn test_set_none() {
        let value = RespValue::Set(None);
        assert_eq!(value.as_bytes(), b"~-1\r\n");
    }

    #[test]
    fn test_push_empty() {
        let value = RespValue::Push(Some(vec![]));
        assert_eq!(value.as_bytes(), b">0\r\n");
    }

    #[test]
    fn test_push_none() {
        let value = RespValue::Push(None);
        assert_eq!(value.as_bytes(), b">-1\r\n");
    }

    #[test]
    fn test_is_none_bulk_string() {
        let value = RespValue::BulkString(Some(Cow::Borrowed("")));
        assert!(value.is_none());

        let value = RespValue::BulkString(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_is_none_array() {
        let value = RespValue::Array(Some(vec![]));
        assert!(value.is_none());

        let value = RespValue::Array(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_is_none_map() {
        let value = RespValue::Map(Some(vec![]));
        assert!(value.is_none());

        let value = RespValue::Map(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_is_none_set() {
        let value = RespValue::Set(Some(vec![]));
        assert!(value.is_none());

        let value = RespValue::Set(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_is_none_push() {
        let value = RespValue::Push(Some(vec![]));
        assert!(value.is_none());

        let value = RespValue::Push(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_is_none_verbatim_string() {
        let value = RespValue::VerbatimString(Some(Cow::Borrowed("")));
        assert!(value.is_none());

        let value = RespValue::VerbatimString(None);
        assert!(value.is_none());
    }

    #[test]
    fn test_from_big_number() {
        let value: RespValue = RespValue::BigNumber(Cow::Borrowed("12345"));
        assert_eq!(value.as_bytes(), b"(12345\r\n");
    }

    #[test]
    fn test_from_bulk_error() {
        let value: RespValue = RespValue::BulkError(Some(Cow::Borrowed("error")));
        assert_eq!(value.as_bytes(), b"!error\r\n");

        let value: RespValue = RespValue::BulkError(None);
        assert_eq!(value.as_bytes(), b"!-1\r\n");
    }

    #[test]
    fn test_from_verbatim_string() {
        let value: RespValue = RespValue::VerbatimString(Some(Cow::Borrowed("verbatim")));
        assert_eq!(value.as_bytes(), b"=verbatim\r\n");

        let value: RespValue = RespValue::VerbatimString(None);
        assert_eq!(value.as_bytes(), b"=-1\r\n");
    }

    #[test]
    fn test_from_map() {
        let value: RespValue = RespValue::Map(Some(vec![
            (
                RespValue::SimpleString(Cow::Borrowed("key1")),
                RespValue::Integer(123),
            ),
            (
                RespValue::SimpleString(Cow::Borrowed("key2")),
                RespValue::BulkString(Some(Cow::Borrowed("value"))),
            ),
        ]));
        assert_eq!(
            value.as_bytes(),
            b"%2\r\n+key1\r\n:123\r\n+key2\r\n$5\r\nvalue\r\n"
        );

        let value: RespValue = RespValue::Map(None);
        assert_eq!(value.as_bytes(), b"%-1\r\n");
    }

    #[test]
    fn test_from_set() {
        let value: RespValue = RespValue::Set(Some(vec![
            RespValue::Integer(1),
            RespValue::SimpleString(Cow::Borrowed("two")),
            RespValue::BulkString(Some(Cow::Borrowed("three"))),
        ]));
        assert_eq!(value.as_bytes(), b"~3\r\n:1\r\n+two\r\n$5\r\nthree\r\n");

        let value: RespValue = RespValue::Set(None);
        assert_eq!(value.as_bytes(), b"~-1\r\n");
    }

    #[test]
    fn test_from_push() {
        let value: RespValue = RespValue::Push(Some(vec![
            RespValue::SimpleString(Cow::Borrowed("message")),
            RespValue::Integer(42),
        ]));
        assert_eq!(value.as_bytes(), b">2\r\n+message\r\n:42\r\n");

        let value: RespValue = RespValue::Push(None);
        assert_eq!(value.as_bytes(), b">-1\r\n");
    }
}
