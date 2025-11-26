use std::{collections::BTreeMap, fmt::Display, iter::Peekable, path::PathBuf};

use thiserror::Error;

const INT_PREFIX: u8 = b'i';
const INT_SUFFIX: u8 = b'e';
const LIST_PREFIX: u8 = b'l';
const LIST_SUFFIX: u8 = b'e';
const DICTIONARY_PREFIX: u8 = b'd';
const DICTIONARY_SUFFIX: u8 = b'e';
const STRING_DELIMITER: u8 = b':';

const ERROR_MISSING_PREFIX: &str = "Missing prefix value";
const ERROR_MISSING_SUFFIX: &str = "Missing suffix value";
const ERROR_INVALID_INTEGER: &str = "Invalid integer";
const ERROR_NON_NUMERIC_CHARACTER: &str = "Non-numeric character in integer";
const ERROR_NEGATIVE_ZERO: &str = "-0 is an invalid integer";
const ERROR_NOT_ENOUGH_CHARS: &str = "Not enough characters";
const ERROR_INVALID_KEY: &str = "Invalid key. Keys must be of type String";
const ERROR_INVALID_UTF8: &str = "Error converting bytes to UTF8";
const ERROR_INVALID_DICT: &str = "Invalid dictionary";

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BencodeType {
    Integer(i64),
    List(Vec<BencodeType>),
    Dictionary(BencodeMap),
    String(Vec<u8>),
}

#[derive(Debug, Error)]
pub enum BencodeGetErr {
    #[error("Invalid type, must be string")]
    InvalidType,
    #[error("Invalid UTF-8 String")]
    InvalidUtf8,
    #[error("Invalid conversion to provided type")]
    InvalidConversion,
}

pub type BencodeMap = BTreeMap<Vec<u8>, BencodeType>;

impl TryFrom<&BencodeType> for String {
    type Error = BencodeGetErr;
    fn try_from(value: &BencodeType) -> Result<Self, Self::Error> {
        value.get_utf8_string()
    }
}

impl TryFrom<&BencodeType> for i64 {
    type Error = BencodeGetErr;
    fn try_from(value: &BencodeType) -> Result<Self, Self::Error> {
        match value {
            BencodeType::Integer(x) => Ok(*x),
            _ => Err(BencodeGetErr::InvalidConversion),
        }
    }
}

impl TryFrom<&BencodeType> for BencodeMap {
    type Error = BencodeGetErr;
    fn try_from(value: &BencodeType) -> Result<Self, Self::Error> {
        match value {
            BencodeType::Dictionary(x) => Ok(x.clone()),
            _ => Err(BencodeGetErr::InvalidConversion),
        }
    }
}

impl<'a, T> TryFrom<&'a BencodeType> for Vec<T>
where
    T: TryFrom<&'a BencodeType, Error = BencodeGetErr>,
{
    type Error = BencodeGetErr;
    fn try_from(value: &'a BencodeType) -> Result<Self, Self::Error> {
        match value {
            BencodeType::List(x) => x.iter().map(T::try_from).collect::<Result<Vec<T>, _>>(),
            _ => Err(BencodeGetErr::InvalidConversion),
        }
    }
}

impl<'a> TryFrom<&'a BencodeType> for Vec<u8> {
    type Error = BencodeGetErr;
    fn try_from(value: &'a BencodeType) -> Result<Self, Self::Error> {
        match value {
            BencodeType::String(x) => Ok(x.clone()),
            _ => Err(BencodeGetErr::InvalidConversion),
        }
    }
}

impl<'a> TryFrom<&'a BencodeType> for PathBuf {
    type Error = BencodeGetErr;
    fn try_from(value: &'a BencodeType) -> Result<Self, Self::Error> {
        match value {
            BencodeType::String(_) => Ok(PathBuf::from(value.get_utf8_string()?)),
            _ => Err(BencodeGetErr::InvalidConversion),
        }
    }
}

// TODO: update T to accept TryInfo instead;
// https://doc.rust-lang.org/std/convert/trait.TryFrom.html
pub trait BencodeMapDecoder {
    fn get_decode<'a, T>(&'a self, key: &str) -> Option<T>
    where
        T: TryFrom<&'a BencodeType>;
    fn try_decode(bytes: &[u8]) -> Result<BencodeMap, BencodeParseErr>;
    fn print_keys(&self);
}

impl BencodeMapDecoder for BencodeMap {
    fn get_decode<'a, T>(&'a self, key: &str) -> Option<T>
    where
        T: TryFrom<&'a BencodeType>,
    {
        let str_as_bytes = key.as_bytes();
        match self.get(str_as_bytes) {
            Some(x) => T::try_from(x).ok(),
            _ => None,
        }
    }

    fn try_decode(bytes: &[u8]) -> Result<BencodeMap, BencodeParseErr> {
        match bytes.first() {
            Some(_) => match read_dictionary(&mut bytes.iter().cloned().peekable().clone())? {
                BencodeType::Dictionary(x) => Ok(x),
                _ => Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
                    ERROR_INVALID_DICT,
                ))),
            },
            None => Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
                ERROR_MISSING_PREFIX,
            ))),
        }
    }

    fn print_keys(&self) {
        let iter = self.keys();
        for x in iter {
            if let Ok(y) = String::from_utf8(x.clone()) {
                println!("{y}");
            } else {
                println!("{}", ERROR_INVALID_KEY);
            }
        }
    }
}

pub trait BencodeMapEncoder {
    fn get_encode(&self) -> Vec<u8>;
}

impl BencodeMapEncoder for BencodeMap {
    fn get_encode(&self) -> Vec<u8> {
        let wrapper = BencodeType::Dictionary(self.clone());
        encode(&wrapper)
    }
}

impl Display for BencodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BencodeType::Integer(x) => write!(f, "Integer({x})"),
            BencodeType::String(x) => match String::from_utf8(x.clone()) {
                Ok(utf8) => write!(f, "String({utf8})"),
                Err(b) => write!(f, "String({b})"),
            },
            BencodeType::Dictionary(x) => write!(f, "Dictionary({x:?})"),
            BencodeType::List(x) => write!(f, "List({x:?}"),
        }
    }
}

impl BencodeType {
    pub fn get_string(&self) -> Result<Vec<u8>, BencodeGetErr> {
        match self {
            Self::String(x) => Ok(x.clone()),
            _ => Err(BencodeGetErr::InvalidType),
        }
    }

    pub fn get_utf8_string(&self) -> Result<String, BencodeGetErr> {
        match self {
            Self::String(x) => String::from_utf8(x.clone()).map_err(|_| BencodeGetErr::InvalidUtf8),
            _ => Err(BencodeGetErr::InvalidUtf8),
        }
    }
}

#[derive(Debug, PartialEq, Error)]
pub enum BencodeParseErr {
    #[error("Empty bencode")]
    EmptyBencode,
    #[error("Invalid bencode type found")]
    InvalidBencode(String),
    #[error("Invalid integer bencode type found")]
    InvalidIntegerBencode(String),
    #[error("Invalid list bencode type found")]
    InvalidListBencode(String),
    #[error("Invalid dictionary bencode type found")]
    InvalidDictionaryBencode(String),
    #[error("Invalid string bencode type found")]
    InvalidStringBencode(String),
}

pub fn decode_to_vec(encoded_value: &[u8]) -> Result<Vec<BencodeType>, BencodeParseErr> {
    let mut vec: Vec<BencodeType> = Vec::new();

    let mut iter = encoded_value.iter().copied().peekable();

    while iter.peek().is_some() {
        vec.push(read_value(&mut iter)?);
    }

    Ok(vec)
}

fn read_value(
    iter: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<BencodeType, BencodeParseErr> {
    if let Some(c) = iter.peek() {
        match c {
            &INT_PREFIX => read_integer(iter),
            &LIST_PREFIX => read_list(iter),
            &DICTIONARY_PREFIX => read_dictionary(iter),
            b'0'..=b'9' => read_string(iter),
            _ => Err(BencodeParseErr::InvalidBencode(c.to_string())),
        }
    } else {
        Err(BencodeParseErr::EmptyBencode)
    }
}

fn read_integer(iter: &mut impl Iterator<Item = u8>) -> Result<BencodeType, BencodeParseErr> {
    let mut temp = String::new();

    for x in iter.by_ref() {
        match x {
            b'-' => temp.push(char::from(x)),
            b'0'..=b'9' => temp.push(char::from(x)),
            INT_PREFIX => continue,
            INT_SUFFIX => break,
            _ => {
                return Err(BencodeParseErr::InvalidIntegerBencode(String::from(
                    ERROR_NON_NUMERIC_CHARACTER,
                )))
            }
        }
    }

    if temp == "-0" {
        return Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_NEGATIVE_ZERO,
        )));
    }

    temp.parse()
        .map(BencodeType::Integer)
        .map_err(|_| BencodeParseErr::InvalidIntegerBencode(String::from(ERROR_INVALID_INTEGER)))
}

fn read_list(
    iter: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<BencodeType, BencodeParseErr> {
    match iter.next() {
        Some(x) if x == LIST_PREFIX => {}
        _ => {
            return Err(BencodeParseErr::InvalidListBencode(String::from(
                ERROR_MISSING_PREFIX,
            )));
        }
    }

    let mut result: Vec<BencodeType> = Vec::new();
    while let Some(x) = iter.peek() {
        match x {
            &LIST_SUFFIX => {
                iter.next();
                return Ok(BencodeType::List(result));
            }
            _ => {
                result.push(read_value(iter)?);
            }
        }
    }

    Err(BencodeParseErr::InvalidListBencode(String::from(
        ERROR_MISSING_SUFFIX,
    )))
}

fn read_dictionary(
    iter: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<BencodeType, BencodeParseErr> {
    match iter.next() {
        Some(x) if x == DICTIONARY_PREFIX => {}
        _ => {
            return Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
                ERROR_MISSING_PREFIX,
            )));
        }
    }

    let mut result: BencodeMap = BencodeMap::new();
    while let Some(x) = iter.peek() {
        match x {
            &DICTIONARY_SUFFIX => {
                iter.next();
                return Ok(BencodeType::Dictionary(result));
            }
            b'0'..=b'9' => {
                let key = read_value(iter)?.get_string().map_err(|_| {
                    BencodeParseErr::InvalidDictionaryBencode(String::from(ERROR_INVALID_KEY))
                })?;
                let value = read_value(iter)?;
                result.insert(key, value);
            }
            _ => {
                return Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
                    ERROR_INVALID_KEY,
                )));
            }
        }
    }

    Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
        ERROR_MISSING_SUFFIX,
    )))
}

fn read_string(iter: &mut impl Iterator<Item = u8>) -> Result<BencodeType, BencodeParseErr> {
    let len_str = String::from_utf8(iter.take_while(|&ch| ch != STRING_DELIMITER).collect())
        .map_err(|_| BencodeParseErr::InvalidStringBencode(String::from(ERROR_INVALID_UTF8)))?;

    if len_str.is_empty() {
        return Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));
    }

    let len: usize = len_str.parse().map_err(|_| {
        BencodeParseErr::InvalidStringBencode(String::from(ERROR_NON_NUMERIC_CHARACTER))
    })?;

    let result: Vec<u8> = iter.take(len).collect();

    if result.len() != len {
        return Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));
    }

    Ok(BencodeType::String(result))
}

fn encode_string(bytes: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(bytes.len().to_string().as_bytes());
    buffer.push(STRING_DELIMITER);
    buffer.extend_from_slice(bytes);

    buffer
}

pub fn encode(value: &BencodeType) -> Vec<u8> {
    let mut buffer = Vec::new();

    match value {
        BencodeType::Integer(x) => {
            buffer.push(INT_PREFIX);
            buffer.extend_from_slice(x.to_string().as_bytes());
            buffer.push(INT_SUFFIX);
        }
        BencodeType::String(x) => {
            buffer.extend_from_slice(&encode_string(x));
        }
        BencodeType::List(x) => {
            buffer.push(LIST_PREFIX);
            for item in x {
                buffer.extend_from_slice(&encode(item));
            }
            buffer.push(LIST_SUFFIX);
        }
        BencodeType::Dictionary(x) => {
            buffer.push(DICTIONARY_PREFIX);
            for (key, value) in x {
                buffer.extend_from_slice(&encode_string(key));
                buffer.extend_from_slice(&encode(value));
            }
            buffer.push(DICTIONARY_SUFFIX);
        }
    }

    buffer
}

pub fn encode_vec(values: &Vec<BencodeType>) -> Vec<u8> {
    let mut buffer = Vec::new();

    for x in values {
        buffer.extend_from_slice(&encode(x));
    }

    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    // INTEGER READ TESTS
    #[test]
    fn read_integer_success() {
        let mut data = "i3e".bytes().into_iter();
        let expected = Ok(BencodeType::Integer(3));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_invalid_format() {
        let mut data = "ie3".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_INVALID_INTEGER,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_non_numeric() {
        let mut data = "i0te".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_NON_NUMERIC_CHARACTER,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_neg_zero() {
        let mut data = "i-0e".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_NEGATIVE_ZERO,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    // STRING READ TESTS
    #[test]
    fn read_string_success() {
        let mut data = "6:pieces".bytes().into_iter();
        let expected = Ok(BencodeType::String(String::from("pieces").into_bytes()));
        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_no_chars() {
        let mut data = "0:".bytes().into_iter();
        let expected = Ok(BencodeType::String(String::from("").into_bytes()));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_invalid_len_char() {
        let mut data = "4r:test".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NON_NUMERIC_CHARACTER,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_not_enough_chars() {
        let mut data = "4:hi".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_no_len() {
        let mut data = ":hi".bytes().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    // LIST READ TESTS
    #[test]
    fn read_list_success() {
        let mut data = "l4:spam4:eggse".bytes().into_iter().peekable();
        let expected = Ok(BencodeType::List(vec![
            BencodeType::String(String::from("spam").into_bytes()),
            BencodeType::String(String::from("eggs").into_bytes()),
        ]));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_nested() {
        let mut data = "l4:spaml4:eggsee".bytes().into_iter().peekable();
        let expected = Ok(BencodeType::List(vec![
            BencodeType::String(String::from("spam").into_bytes()),
            BencodeType::List(vec![BencodeType::String(String::from("eggs").into_bytes())]),
        ]));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_invalid_string() {
        let mut data = "l24:spam4:eggse".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_invalid_bencode() {
        let mut data = "lx23e".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidBencode(b'x'.to_string()));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_missing_prefix() {
        let mut data = "i2ee".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidListBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_missing_suffix() {
        let mut data = "li2e".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidListBencode(String::from(
            ERROR_MISSING_SUFFIX,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    // DICTIONARY READ TESTS
    #[test]
    fn read_dictionary_success() {
        let mut data = "d3:cow3:moo4:spam4:eggse".bytes().into_iter().peekable();
        let mut map: BencodeMap = BencodeMap::new();
        map.insert(
            String::from("cow").into_bytes(),
            BencodeType::String(String::from("moo").into_bytes()),
        );
        map.insert(
            String::from("spam").into_bytes(),
            BencodeType::String(String::from("eggs").into_bytes()),
        );
        let expected = Ok(BencodeType::Dictionary(map));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_dictionary_nested_map() {
        let mut data = "d3:cow3:moo4:spam4:eggs4:dictd3:key5:valueee"
            .bytes()
            .into_iter()
            .peekable();

        let mut map: BencodeMap = BencodeMap::new();
        map.insert(
            String::from("cow").into_bytes(),
            BencodeType::String(String::from("moo").into_bytes()),
        );
        map.insert(
            String::from("spam").into_bytes(),
            BencodeType::String(String::from("eggs").into_bytes()),
        );

        let mut nested: BencodeMap = BencodeMap::new();
        nested.insert(
            String::from("key").into_bytes(),
            BencodeType::String(String::from("value").into_bytes()),
        );
        map.insert(
            String::from("dict").into_bytes(),
            BencodeType::Dictionary(nested),
        );

        let expected = Ok(BencodeType::Dictionary(map));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_dictionary_missing_prefix() {
        let mut data = "3:cow3:moo4:spam4:eggse".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_dictionary_invalid_key() {
        let mut data = "die33:moo4:spam4:eggse".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
            ERROR_INVALID_KEY,
        )));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_dictionary_missing_suffix() {
        let mut data = "d3:cow3:moo4:spam4:eggs".bytes().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidDictionaryBencode(String::from(
            ERROR_MISSING_SUFFIX,
        )));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }
}
