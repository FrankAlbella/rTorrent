use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    iter::Peekable,
};

const INT_PREFIX: &char = &'i';
const INT_SUFFIX: &char = &'e';
const LIST_PREFIX: &char = &'l';
const LIST_SUFFIX: &char = &'e';
const DICTIONARY_PREFIX: &char = &'d';
const DICTIONARY_SUFFIX: &char = &'e';
const STRING_DELIMITER: &char = &':';

const ERROR_MISSING_PREFIX: &str = "Missing prefix value";
const ERROR_MISSING_SUFFIX: &str = "Missing suffix value";
const ERROR_INVALID_INTEGER: &str = "Invalid integer";
const ERROR_NON_NUMERIC_CHARACTER: &str = "Non-numeric character in integer";
const ERROR_NEGATIVE_ZERO: &str = "-0 is an invalid integer";
const ERROR_NOT_ENOUGH_CHARS: &str = "Not enough characters";
const ERROR_INVALID_KEY: &str = "Invalid key. Keys must be of type String";

#[derive(Debug)]
pub enum BencodeType {
    Integer(i64),
    List(Vec<BencodeType>),
    Dictionary(HashMap<BencodeType, BencodeType>),
    String(String),
}

impl PartialEq for BencodeType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (BencodeType::Integer(x), BencodeType::Integer(y)) => x == y,
            (BencodeType::List(x), BencodeType::List(y)) => x == y,
            (BencodeType::Dictionary(x), BencodeType::Dictionary(y)) => {
                if x.len() != y.len() {
                    return false;
                }

                x.iter().all(|(key, value)| y.get(key) == Some(value))
            }
            (BencodeType::String(x), BencodeType::String(y)) => x == y,
            _ => false,
        }
    }
}

impl Hash for BencodeType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            BencodeType::Integer(x) => {
                state.write_u8(0);
                x.hash(state);
            }
            BencodeType::List(x) => {
                state.write_u8(1);
                x.hash(state);
            }
            BencodeType::Dictionary(x) => {
                state.write_u8(2);
                let mut kvs: Vec<_> = x.iter().collect();
                kvs.sort_by_key(|(k, _)| {
                    let mut hasher = DefaultHasher::new();
                    k.hash(&mut hasher);
                    hasher.finish()
                });
                for (k, v) in kvs {
                    k.hash(state);
                    v.hash(state);
                }
            }
            BencodeType::String(x) => {
                state.write_u8(3);
                x.hash(state);
            }
        }
    }
}

impl Eq for BencodeType {}

#[derive(Debug, PartialEq)]
pub enum BencodeParseErr {
    EmptyBencode,
    InvalidBencode(String),
    InvalidIntegerBencode(String),
    InvalidListBencode(String),
    InvalidDictionaryBencode(String),
    InvalidStringBencode(String),
}

pub fn decode_to_vec(encoded_value: &str) -> Result<Vec<BencodeType>, BencodeParseErr> {
    let mut vec: Vec<BencodeType> = Vec::new();

    let mut iter = encoded_value.chars().into_iter().peekable();

    while let Some(_) = iter.peek() {
        vec.push(read_value(&mut iter)?);
    }

    Ok(vec)
}

fn read_value(
    iter: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<BencodeType, BencodeParseErr> {
    if let Some(c) = iter.peek() {
        match c {
            INT_PREFIX => read_integer(iter),
            LIST_PREFIX => read_list(iter),
            DICTIONARY_PREFIX => read_dictionary(iter),
            '0'..='9' => read_string(iter),
            _ => Err(BencodeParseErr::InvalidBencode(c.to_string())),
        }
    } else {
        Err(BencodeParseErr::EmptyBencode)
    }
}

fn read_integer(iter: &mut impl Iterator<Item = char>) -> Result<BencodeType, BencodeParseErr> {
    let mut temp = String::new();

    while let Some(x) = iter.next() {
        match &x {
            '-' => temp.push(x),
            '0'..='9' => temp.push(x),
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
    iter: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<BencodeType, BencodeParseErr> {
    match iter.next() {
        Some(x) if &x == LIST_PREFIX => {}
        _ => {
            return Err(BencodeParseErr::InvalidListBencode(String::from(
                ERROR_MISSING_PREFIX,
            )));
        }
    }

    let mut result: Vec<BencodeType> = Vec::new();
    while let Some(x) = iter.peek() {
        match x {
            LIST_SUFFIX => {
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
    iter: &mut Peekable<impl Iterator<Item = char>>,
) -> Result<BencodeType, BencodeParseErr> {
    match iter.next() {
        Some(x) if &x == DICTIONARY_PREFIX => {}
        _ => {
            return Err(BencodeParseErr::InvalidListBencode(String::from(
                ERROR_MISSING_PREFIX,
            )));
        }
    }

    let mut result: HashMap<BencodeType, BencodeType> = HashMap::new();
    while let Some(x) = iter.peek() {
        match x {
            DICTIONARY_SUFFIX => {
                iter.next();
                return Ok(BencodeType::Dictionary(result));
            }
            '0'..='9' => {
                let key = read_value(iter)?;
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

fn read_string(iter: &mut impl Iterator<Item = char>) -> Result<BencodeType, BencodeParseErr> {
    let len_str: String = iter.take_while(|&ch| ch != *STRING_DELIMITER).collect();

    if len_str.is_empty() {
        return Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));
    }

    let len: usize = len_str.parse().map_err(|_| {
        BencodeParseErr::InvalidStringBencode(String::from(ERROR_NON_NUMERIC_CHARACTER))
    })?;

    let result: String = iter.take(len).collect();

    if result.len() != len {
        return Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));
    }

    Ok(BencodeType::String(result))
}

pub fn encode(value: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // INTEGER READ TESTS
    #[test]
    fn read_integer_success() {
        let mut data = "i3e".chars().into_iter();
        let expected = Ok(BencodeType::Integer(3));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_invalid_format() {
        let mut data = "ie3".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_INVALID_INTEGER,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_non_numeric() {
        let mut data = "i0te".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_NON_NUMERIC_CHARACTER,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_integer_neg_zero() {
        let mut data = "i-0e".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidIntegerBencode(String::from(
            ERROR_NEGATIVE_ZERO,
        )));

        let result = read_integer(&mut data);

        assert_eq!(result, expected)
    }

    // STRING READ TESTS
    #[test]
    fn read_string_success() {
        let mut data = "4:test".chars().into_iter();
        let expected = Ok(BencodeType::String(String::from("test")));
        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_no_chars() {
        let mut data = "0:".chars().into_iter();
        let expected = Ok(BencodeType::String(String::from("")));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_invalid_len_char() {
        let mut data = "4r:test".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NON_NUMERIC_CHARACTER,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_not_enough_chars() {
        let mut data = "4:hi".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_string_no_len() {
        let mut data = ":hi".chars().into_iter();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));

        let result = read_string(&mut data);

        assert_eq!(result, expected)
    }

    // LIST READ TESTS
    #[test]
    fn read_list_success() {
        let mut data = "l4:spam4:eggse".chars().into_iter().peekable();
        let expected = Ok(BencodeType::List(vec![
            BencodeType::String(String::from("spam")),
            BencodeType::String(String::from("eggs")),
        ]));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_nested() {
        let mut data = "l4:spaml4:eggsee".chars().into_iter().peekable();
        let expected = Ok(BencodeType::List(vec![
            BencodeType::String(String::from("spam")),
            BencodeType::List(vec![BencodeType::String(String::from("eggs"))]),
        ]));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_invalid_string() {
        let mut data = "l24:spam4:eggse".chars().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidStringBencode(String::from(
            ERROR_NOT_ENOUGH_CHARS,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_invalid_bencode() {
        let mut data = "lx23e".chars().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidBencode(String::from("x")));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_missing_prefix() {
        let mut data = "i2ee".chars().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidListBencode(String::from(
            ERROR_MISSING_PREFIX,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    #[test]
    fn read_list_missing_suffix() {
        let mut data = "li2e".chars().into_iter().peekable();
        let expected = Err(BencodeParseErr::InvalidListBencode(String::from(
            ERROR_MISSING_SUFFIX,
        )));

        let result = read_list(&mut data);

        assert_eq!(result, expected)
    }

    // DICTIONARY READ TESTS
    #[test]
    fn read_dictionary_success() {
        let mut data = "d3:cow3:moo4:spam4:eggse".chars().into_iter().peekable();
        let mut map: HashMap<BencodeType, BencodeType> = HashMap::new();
        map.insert(
            BencodeType::String(String::from("cow")),
            BencodeType::String(String::from("moo")),
        );
        map.insert(
            BencodeType::String(String::from("spam")),
            BencodeType::String(String::from("eggs")),
        );
        let expected = Ok(BencodeType::Dictionary(map));

        let result = read_dictionary(&mut data);

        assert_eq!(result, expected)
    }
}
