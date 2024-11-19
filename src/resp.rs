use std::borrow::Cow;

// todo add is_null method for each type
#[derive(Debug, Clone)]
pub enum RespValue<'a> {
    SimpleString(Cow<'a, str>),
    Error(Cow<'a, str>),
    Integer(i64),
    BulkString(Option<Cow<'a, str>>),
    Array(Option<Vec<RespValue<'a>>>),
    Null,
    Boolean(bool),
    Double(f64),
    BigNumber(Cow<'a, str>),
    BulkError(Option<Cow<'a, str>>),
    VerbatimString(Option<Cow<'a, str>>),
    Map(Option<Vec<(RespValue<'a>, RespValue<'a>)>>),
    Set(Option<Vec<RespValue<'a>>>),
    Push(Option<Vec<RespValue<'a>>>),
}

impl PartialEq for RespValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RespValue::SimpleString(a), RespValue::SimpleString(b)) => *a == *b,
            (RespValue::Error(a), RespValue::Error(b)) => *a == *b,
            (RespValue::Integer(a), RespValue::Integer(b)) => a == b,
            (RespValue::BulkString(a), RespValue::BulkString(b)) => *a == *b,
            (RespValue::Array(a), RespValue::Array(b)) => *a == *b,
            (RespValue::Null, RespValue::Null) => true,
            (RespValue::Boolean(a), RespValue::Boolean(b)) => a == b,
            (RespValue::Double(a), RespValue::Double(b)) => a == b,
            (RespValue::BigNumber(a), RespValue::BigNumber(b)) => *a == *b,
            (RespValue::BulkError(a), RespValue::BulkError(b)) => *a == *b,
            (RespValue::VerbatimString(a), RespValue::VerbatimString(b)) => *a == *b,
            (RespValue::Map(a), RespValue::Map(b)) => *a == *b,
            (RespValue::Set(a), RespValue::Set(b)) => *a == *b,
            (RespValue::Push(a), RespValue::Push(b)) => *a == *b,
            _ => false,
        }
    }
}

// Implement From and Into traits for RespValue
impl From<String> for RespValue<'_> {
    fn from(value: String) -> Self {
        RespValue::SimpleString(Cow::Owned(value))
    }
}

impl<'a> From<&'a str> for RespValue<'a> {
    fn from(value: &'a str) -> Self {
        RespValue::SimpleString(Cow::Borrowed(value))
    }
}

impl From<i64> for RespValue<'_> {
    fn from(value: i64) -> Self {
        RespValue::Integer(value)
    }
}

impl From<Option<String>> for RespValue<'_> {
    fn from(value: Option<String>) -> Self {
        RespValue::BulkString(value.map(Cow::Owned))
    }
}

impl<'a> From<Vec<RespValue<'a>>> for RespValue<'a> {
    fn from(value: Vec<RespValue<'a>>) -> Self {
        RespValue::Array(Some(value))
    }
}

impl From<bool> for RespValue<'_> {
    fn from(value: bool) -> Self {
        RespValue::Boolean(value)
    }
}

impl From<f64> for RespValue<'_> {
    fn from(value: f64) -> Self {
        RespValue::Double(value)
    }
}

impl<'a> From<(RespValue<'a>, RespValue<'a>)> for RespValue<'a> {
    fn from(value: (RespValue<'a>, RespValue<'a>)) -> Self {
        RespValue::Map(Some(vec![value]))
    }
}

impl<'a> From<Vec<(RespValue<'a>, RespValue<'a>)>> for RespValue<'a> {
    fn from(value: Vec<(RespValue<'a>, RespValue<'a>)>) -> Self {
        RespValue::Map(Some(value))
    }
}

// Implement Into traits for RespValue
impl Into<String> for RespValue<'_> {
    fn into(self) -> String {
        match self {
            RespValue::SimpleString(value) => value.into_owned(),
            _ => panic!("Cannot convert {:?} to String", self),
        }
    }
}

impl Into<i64> for RespValue<'_> {
    fn into(self) -> i64 {
        match self {
            RespValue::Integer(value) => value,
            _ => panic!("Cannot convert {:?} to i64", self),
        }
    }
}

impl Into<Option<String>> for RespValue<'_> {
    fn into(self) -> Option<String> {
        match self {
            RespValue::BulkString(value) => value.map(|v| v.into_owned()),
            _ => panic!("Cannot convert {:?} to Option<String>", self),
        }
    }
}

impl<'a> Into<Vec<RespValue<'a>>> for RespValue<'a> {
    fn into(self) -> Vec<RespValue<'a>> {
        match self {
            RespValue::Array(value) => value.unwrap().clone(),
            RespValue::Set(value) => value.unwrap().clone(),
            RespValue::Push(value) => value.unwrap().clone(),
            _ => panic!("Cannot convert {:?} to Vec<RespValue>", self),
        }
    }
}

impl<'a> From<RespValue<'a>> for Vec<u8> {
    fn from(value: RespValue<'a>) -> Vec<u8> {
        match value {
            RespValue::SimpleString(s) => format!("+{}\r\n", s.to_owned()).into_bytes(),
            RespValue::Error(msg) => format!("-{}\r\n", msg.to_owned()).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::BulkString(s) => match s {
                Some(s) => format!("${}\r\n{}\r\n", s.len(), s.to_owned()).into_bytes(),
                None => "$-1\r\n".as_bytes().to_vec(),
            },
            RespValue::Null => "$-1\r\n".as_bytes().to_vec(),
            RespValue::Array(arr) => {
                let mut bytes = match &arr {
                    Some(a) => format!("*{}\r\n", a.len()).into_bytes(),
                    None => return "*-1\r\n".as_bytes().to_vec(),
                };
                if let Some(values) = arr {
                    for value in values {
                        bytes.extend(value.as_bytes());
                    }
                }
                bytes
            }
            _ => panic!("Cannot convert {:?} to Vec<u8>", value),
        }
    }
}

impl Into<bool> for RespValue<'_> {
    fn into(self) -> bool {
        match self {
            RespValue::Boolean(value) => value,
            _ => panic!("Cannot convert {:?} to bool", self),
        }
    }
}

impl Into<f64> for RespValue<'_> {
    fn into(self) -> f64 {
        match self {
            RespValue::Double(value) => value,
            _ => panic!("Cannot convert {:?} to f64", self),
        }
    }
}

impl<'a> Into<Vec<(RespValue<'a>, RespValue<'a>)>> for RespValue<'a> {
    fn into(self) -> Vec<(RespValue<'a>, RespValue<'a>)> {
        match self {
            RespValue::Map(value) => value.unwrap().clone(),
            _ => panic!("Cannot convert {:?} to Vec<(RespValue, RespValue)>", self),
        }
    }
}

impl RespValue<'_> {
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            RespValue::Error(e) => format!("-{}\r\n", e).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::BulkString(Some(s)) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
            RespValue::BulkString(None) => "$-1\r\n".as_bytes().to_vec(),
            RespValue::Array(Some(arr)) => {
                let mut bytes = format!("*{}\r\n", arr.len()).into_bytes();
                for item in arr {
                    bytes.extend(item.as_bytes());
                }
                bytes
            }
            RespValue::Array(None) => "*-1\r\n".as_bytes().to_vec(),
            RespValue::Null => "_\r\n".as_bytes().to_vec(),
            RespValue::Boolean(b) => format!("#{}\r\n", if *b { "t" } else { "f" }).into_bytes(),
            RespValue::Double(d) => format!(",{}\r\n", d).into_bytes(),
            RespValue::BigNumber(n) => format!("({}\r\n", n).into_bytes(),
            RespValue::BulkError(Some(e)) => format!("!{}\r\n", e).into_bytes(),
            RespValue::BulkError(None) => "!-1\r\n".as_bytes().to_vec(),
            RespValue::VerbatimString(Some(s)) => format!("={}\r\n", s).into_bytes(),
            RespValue::VerbatimString(None) => "=-1\r\n".as_bytes().to_vec(),
            RespValue::Map(Some(m)) => {
                let mut bytes = format!("%{}\r\n", m.len()).into_bytes();
                for (k, v) in m {
                    bytes.extend(k.as_bytes());
                    bytes.extend(v.as_bytes());
                }
                bytes
            }
            RespValue::Map(None) => "%-1\r\n".as_bytes().to_vec(),
            RespValue::Set(Some(s)) => {
                let mut bytes = format!("~{}\r\n", s.len()).into_bytes();
                for item in s {
                    bytes.extend(item.as_bytes());
                }
                bytes
            }
            RespValue::Set(None) => "~-1\r\n".as_bytes().to_vec(),
            RespValue::Push(Some(p)) => {
                let mut bytes = format!(">{}\r\n", p.len()).as_bytes().to_vec();
                for item in p {
                    bytes.extend(item.as_bytes());
                }
                bytes
            }
            RespValue::Push(None) => ">-1\r\n".as_bytes().to_vec(),
        }
    }

    pub fn into_owned(self) -> RespValue<'static> {
        match self {
            RespValue::SimpleString(s) => RespValue::SimpleString(Cow::Owned(s.into_owned())),
            RespValue::Error(e) => RespValue::Error(Cow::Owned(e.into_owned())),
            RespValue::Integer(i) => RespValue::Integer(i),
            RespValue::BulkString(s) => {
                RespValue::BulkString(s.map(|s| Cow::Owned(s.into_owned())))
            }
            RespValue::Array(arr) => {
                RespValue::Array(arr.map(|a| a.into_iter().map(|v| v.into_owned()).collect()))
            }
            RespValue::Null => RespValue::Null,
            RespValue::Boolean(b) => RespValue::Boolean(b),
            RespValue::Double(d) => RespValue::Double(d),
            RespValue::BigNumber(n) => RespValue::BigNumber(Cow::Owned(n.into_owned())),
            RespValue::BulkError(e) => RespValue::BulkError(e.map(|e| Cow::Owned(e.into_owned()))),
            RespValue::VerbatimString(s) => {
                RespValue::VerbatimString(s.map(|s| Cow::Owned(s.into_owned())))
            }
            RespValue::Map(m) => RespValue::Map(m.map(|m| {
                m.into_iter()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            })),
            RespValue::Set(s) => {
                RespValue::Set(s.map(|s| s.into_iter().map(|v| v.into_owned()).collect()))
            }
            RespValue::Push(p) => {
                RespValue::Push(p.map(|p| p.into_iter().map(|v| v.into_owned()).collect()))
            }
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            RespValue::SimpleString(_) => false,
            RespValue::Error(_) => false,
            RespValue::Integer(_) => false,
            RespValue::BulkString(value) => {
                value.is_none() || value.as_ref().map_or(false, |s| s.is_empty())
            }
            RespValue::Array(value) => {
                value.is_none() || value.as_ref().map_or(false, |arr| arr.is_empty())
            }
            RespValue::Null => true,
            RespValue::Boolean(_) => false,
            RespValue::Double(_) => false,
            RespValue::BigNumber(_) => false,
            RespValue::VerbatimString(text) => {
                text.is_none() || text.as_ref().map_or(false, |s| s.is_empty())
            }
            RespValue::Map(value) => {
                value.is_none() || value.as_ref().map_or(false, |m| m.is_empty())
            }
            RespValue::Set(value) => {
                value.is_none() || value.as_ref().map_or(false, |s| s.is_empty())
            }
            RespValue::Push(data) => {
                data.is_none() || data.as_ref().map_or(false, |s| s.is_empty())
            }
            RespValue::BulkError(_) => false,
        }
    }
}

//EOF
