#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

use smallvec::SmallVec;
use std::{
    default,
    io::{BufRead, Error, ErrorKind, Read},
};

#[derive(Debug, Default, PartialEq)]
pub struct ParseKey<R: Read> {
    reader: R,
    overflow: Option<u8>,
}

pub trait ReadKey {
    type Output;
    fn read_key(self) -> Self::Output;
}

impl<R: Read> ReadKey for R {
    type Output = ParseKey<R>;

    fn read_key(self) -> Self::Output {
        ParseKey {
            reader: self,
            overflow: None,
        }
    }
}

impl<R: Read> Iterator for ParseKey<R> {
    type Item = std::io::Result<Key>;

    fn next(&mut self) -> Option<Self::Item> {
        let error = Error::new(ErrorKind::Other, "Could not read buffer.");
        let reader = &mut self.reader;

        if self.overflow.is_some() {
            return Some(KeyEvent::parse_key(
                self.overflow.take().unwrap(),
                &mut reader.bytes(),
            ));
        }
        let mut buf = [0u8; 2];
        match reader.read(&mut buf) {
            Ok(0) => return None,
            Ok(1) if buf[0].is_ascii_digit() => Some(Ok(Key::Letter(buf[0] as char))),
            Ok(_) => {
                let input = &mut Some(buf[1]).into_iter();
                let ret_key = {
                    let mut iter = input.map(|byte| Ok(byte)).chain(reader.bytes());
                    Some(KeyEvent::parse_key(buf[0], &mut iter))
                };
                self.overflow = input.next();
                ret_key
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => self.next(),
            Err(err) => Some(Err(error)),
        }
    }
}

pub struct KeyEvent {}

impl KeyEvent {
    pub fn parse_key<I>(item: u8, iter: &mut I) -> std::io::Result<Key>
    where
        I: Iterator<Item = std::io::Result<u8>>,
    {
        let error = Error::new(ErrorKind::Other, "Could not parse key event.");

        match item {
            b'\x1B' => match iter.next() {
                Some(Ok(b'[')) => match iter.next() {
                    Some(Ok(val)) if val.is_ascii_digit() => Ok(Self::parse_cursor_pos(val, iter)?),
                    Some(Ok(val)) => Ok(Self::parse_control_seq(iter)?),
                    _ => return Err(error),
                },
                Some(Ok(b'0')) => match iter.next() {
                    // Function key F1-F4.
                    Some(Ok(val @ b'P'..=b'S')) => Ok(Key::F(1 + val - b'P')),
                    _ => Err(error),
                },
                Some(Ok(letter)) => {
                    let ch = Self::parse_char(letter, iter)?;
                    Ok(Key::AltKey(ch))
                }
                Some(Err(_)) => return Err(error),
                None => Ok(Key::Escape),
            },
            b'\x08' => Ok(Key::Backspace),
            b'\x09' => Ok(Key::Tab('\t')),
            b'\x0A' => Ok(Key::Enter('\n')),
            b'\x7F' => Ok(Key::Delete),
            itm @ b'\x01'..=b'\x1A' => Ok(Key::CtrlKey((itm as u8 - 0x1 + b'a') as char)),
            // TODO: Parse char should only parse utf8. Change key::Letter back to key::Ascii/key::utf8
            itm => match KeyEvent::parse_char(itm, iter) {
                Ok(key) => Ok(Key::Letter(key)),
                Err(err) => Err(err),
            },
        }
    }

    fn parse_control_seq<I>(iter: &mut I) -> std::io::Result<Key>
    where
        I: Iterator<Item = std::io::Result<u8>>,
    {
        let error = Error::new(ErrorKind::Other, "Could not parse escape sequence.");

        Ok(match iter.next() {
            Some(Ok(b'\x1B')) => match iter.next() {
                Some(Ok(b'[')) => match iter.next() {
                    Some(Ok(val @ b'A'..=b'E')) => Key::F(1 + val - b'A'),
                    _ => return Err(error),
                },
                Some(Ok(b'D')) => Key::Left,
                Some(Ok(b'C')) => Key::Right,
                Some(Ok(b'A')) => Key::Up,
                Some(Ok(b'B')) => Key::Down,
                Some(Ok(b'H')) => Key::Home,
                Some(Ok(b'F')) => Key::End,
                Some(Ok(b'Z')) => Key::TabBack,
                _ => return Err(error),
            },
            _ => return Err(error),
        })
    }

    fn parse_char<I>(item: u8, iter: &mut I) -> std::io::Result<char>
    where
        I: Iterator<Item = std::io::Result<u8>>,
    {
        let item_error = std::io::Error::new(ErrorKind::Other, "Could not parse item.");

        if item.is_ascii() {
            return Ok(item as char);
        }
        let error = std::io::Error::new(std::io::ErrorKind::Other, "Invalid Utf8.");
        let bytes = &mut Vec::new();
        bytes.push(item);
        loop {
            match iter.next() {
                Some(Ok(byte)) => {
                    bytes.push(byte);
                    if let Ok(s) = std::str::from_utf8(bytes.as_slice()) {
                        return Ok(s.chars().next().unwrap());
                    }
                    if bytes.len() >= 4 {
                        return Err(error);
                    }
                }
                _ => return Err(error),
            }
        }
    }

    fn parse_cursor_pos<I>(item: u8, iter: &mut I) -> std::io::Result<Key>
    where
        I: Iterator<Item = std::io::Result<u8>>,
    {
        let error = std::io::Error::new(std::io::ErrorKind::Other, "Could not parse cursor pos.");
        let mut pos: Vec<u8> = vec![item];
        let mut ret_val = (0, 0);
        loop {
            match iter.next() {
                Some(Ok(byte)) => {
                    if byte.is_ascii_digit() {
                        pos.push(byte);
                    }
                    if byte == b';' {
                        ret_val.0 = pos.iter().fold(0, |acc, c| acc * 10 + (c - b'0') as u16);

                        pos.clear();
                    }
                }
                _ => break,
            }
        }
        ret_val.1 = pos.iter().fold(0, |acc, c| acc * 10 + (c - b'0') as u16);
        if ret_val.0 != 0 && ret_val.1 != 0 {
            Ok(Key::CursorPos(ret_val))
        } else {
            Err(error)
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Key {
    CursorPos((u16, u16)),
    Letter(char),
    CtrlKey(char),
    AltKey(char),
    Enter(char),
    Tab(char),
    TabBack,
    F(u8),
    Backspace,
    Delete,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Esc,
}
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Key::CursorPos((r, c)) => write!(f, "CursorPos(R: {} C: {})", r, c),
            Key::Letter(ch) => write!(f, "{}", ch),
            Key::CtrlKey(ch) => write!(f, "CtrlKey: {}", ch),
            Key::AltKey(ch) => write!(f, "AltKey: {}", ch),
            Key::Enter(_) => write!(f, "Enter"),
            Key::Tab(_) => write!(f, "Tab"),
            Key::F(num) => write!(f, "F{}", num),
            Key::TabBack => write!(f, "TabBack"),
            Key::Backspace => write!(f, "Backspace"),
            Key::Delete => write!(f, "Delete"),
            Key::Escape => write!(f, "Escape"),
            Key::Left => write!(f, "Left"),
            Key::Right => write!(f, "Right"),
            Key::Up => write!(f, "Up"),
            Key::Down => write!(f, "Down"),
            Key::Home => write!(f, "Home"),
            Key::End => write!(f, "End"),
            Key::PageUp => write!(f, "PageUp"),
            Key::PageDown => write!(f, "PageDown"),
            Key::Insert => write!(f, "Insert"),
            Key::Esc => write!(f, "Esc"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_iterator() {
        // This fails because the cursor pos escape seq is putting the position off in some way.
        let input = "\x61\x1b\x62\x1b[23;23R\x62";
        dbg!(input.len());
        let mut reader = std::io::Cursor::new(input).read_key();
        assert_eq!(reader.next().unwrap().unwrap(), Key::Letter('a'));
        assert_eq!(reader.next().unwrap().unwrap(), Key::AltKey('b'));
        assert_eq!(reader.next().unwrap().unwrap(), Key::CursorPos((23, 23)));
        dbg!(&reader);
        assert_eq!(reader.next().unwrap().unwrap(), Key::Letter('b'));
    }

    #[test]
    fn test_parse_chars() {
        let st = "tE7!éŷ¤£€ù";
        for byte in st.chars() {
            let reader = std::io::Cursor::new(format!("{}", byte));
            let mut iter = reader.bytes();
            assert_eq!(
                KeyEvent::parse_char(iter.next().unwrap().unwrap(), &mut iter).unwrap(),
                byte
            );
        }
    }
    #[test]
    fn test_parse_cursor_pos() {
        let st = "\x1b[23;23R";
        let reader = std::io::Cursor::new(st);
        let mut iter = reader.bytes();
        assert_eq!(
            KeyEvent::parse_key(iter.next().unwrap().unwrap(), &mut iter).unwrap(),
            Key::CursorPos((23, 23))
        )
    }
    #[test]
    fn test_parse_control_seq() {
        let simulated_keys = vec![
            "\x1B\x41", "\x1B\x42", "\x1B\x43", "\x1B\x44", "\x1B\x48", "\x1B\x46",
        ];
        let mut expected = vec![
            Key::Up,
            Key::Down,
            Key::Right,
            Key::Left,
            Key::Home,
            Key::End,
            Key::TabBack,
        ]
        .into_iter();
        for seq in simulated_keys {
            let reader = std::io::Cursor::new(seq);
            let mut iter = reader.bytes();
            let key = KeyEvent::parse_control_seq(&mut iter).unwrap();
            assert_eq!(expected.next().unwrap(), key);
        }
    }

    #[test]
    fn test_alt_keys() {
        let simulated_keys = vec![
            "\x1B\x61", "\x1B\x62", "\x1B\x63", "\x1B\x64", "\x1B\x65", "\x1B\x66", "\x1B\x67",
            "\x1B\x68", "\x1B\x69", "\x1B\x6A", "\x1B\x6B", "\x1B\x6C", "\x1B\x6D", "\x1B\x6E",
            "\x1B\x6F", "\x1B\x70", "\x1B\x71", "\x1B\x72", "\x1B\x73", "\x1B\x74", "\x1B\x75",
            "\x1B\x76", "\x1B\x77", "\x1B\x78", "\x1B\x79",
        ];
        let mut expected = vec![
            Key::AltKey('a'),
            Key::AltKey('b'),
            Key::AltKey('c'),
            Key::AltKey('d'),
            Key::AltKey('e'),
            Key::AltKey('f'),
            Key::AltKey('g'),
            Key::AltKey('h'),
            Key::AltKey('i'),
            Key::AltKey('j'),
            Key::AltKey('k'),
            Key::AltKey('l'),
            Key::AltKey('m'),
            Key::AltKey('n'),
            Key::AltKey('o'),
            Key::AltKey('p'),
            Key::AltKey('q'),
            Key::AltKey('r'),
            Key::AltKey('s'),
            Key::AltKey('t'),
            Key::AltKey('u'),
            Key::AltKey('v'),
            Key::AltKey('w'),
            Key::AltKey('x'),
            Key::AltKey('y'),
            Key::AltKey('z'),
        ]
        .into_iter();

        for seq in simulated_keys {
            let reader = std::io::Cursor::new(seq);
            let mut iter = reader.bytes();
            let key = KeyEvent::parse_key(iter.next().unwrap().unwrap(), &mut iter).unwrap();
            assert_eq!(key, expected.next().unwrap());
        }
    }

    #[test]
    fn test_first_f_keys() {
        let simulated_keys = vec![
            "\x1b\x30\x50",
            "\x1b\x30\x51",
            "\x1b\x30\x52",
            "\x1b\x30\x53",
        ];
        let mut expected =
            vec![Key::F(1), Key::F(2), Key::F(3), Key::F(4), Key::AltKey('a')].into_iter();

        for seq in simulated_keys {
            let reader = std::io::Cursor::new(seq);
            let mut iter = reader.bytes();
            let key = KeyEvent::parse_key(iter.next().unwrap().unwrap(), &mut iter).unwrap();
            assert_eq!(key, expected.next().unwrap());
        }
    }

    #[test]
    fn test_parse_keys() {
        let simulated_keys = vec![
            "\x01", "\x02", "\x03", "\x04", "\x05", "\x06", "\x07", "\x08", "\x09", "\x0A", "\x0B",
            "\x0C", "\x0D", "\x0E", "\x0F", "\x10", "\x11", "\x12", "\x13", "\x14", "\x15", "\x16",
            "\x17", "\x18", "\x19", "\x1A", "\x20", "\x21", "\x22", "\x23", "\x24", "\x25", "\x26",
            "\x28", "\x29", "\x2A", "\x2B", "\x2C", "\x2D", "\x2E", "\x2F", "\x30", "\x31", "\x32",
            "\x33", "\x34", "\x36", "\x37", "\x38", "\x39", "\x3A", "\x3B", "\x3C", "\x3D", "\x3E",
            "\x3F", "\x40", "\x41", "\x42", "\x43", "\x44", "\x45", "\x46", "\x48", "\x49", "\x4A",
            "\x4B", "\x4C", "\x4D", "\x4E", "\x4F", "\x50", "\x51", "\x52", "\x53", "\x54", "\x56",
            "\x57", "\x58", "\x59", "\x5A", "\x5B", "\x5C", "\x5D", "\x5E", "\x5F", "\x60", "\x61",
            "\x62", "\x63", "\x64", "\x65", "\x66", "\x68", "\x69", "\x6A", "\x6B", "\x6C", "\x6D",
            "\x6E", "\x6F", "\x70", "\x71", "\x72", "\x73", "\x74", "\x76", "\x77", "\x78", "\x79",
            "\x7A", "\x7B", "\x7C", "\x7D", "\x7E", "\x7F",
        ];

        let mut expected = vec![
            Key::CtrlKey('a'),
            Key::CtrlKey('b'),
            Key::CtrlKey('c'),
            Key::CtrlKey('d'),
            Key::CtrlKey('e'),
            Key::CtrlKey('f'),
            Key::CtrlKey('g'),
            Key::Backspace,
            Key::Tab('\t'),
            Key::Enter('\n'),
            Key::CtrlKey('k'),
            Key::CtrlKey('l'),
            Key::CtrlKey('m'),
            Key::CtrlKey('n'),
            Key::CtrlKey('o'),
            Key::CtrlKey('p'),
            Key::CtrlKey('q'),
            Key::CtrlKey('r'),
            Key::CtrlKey('s'),
            Key::CtrlKey('t'),
            Key::CtrlKey('u'),
            Key::CtrlKey('v'),
            Key::CtrlKey('w'),
            Key::CtrlKey('x'),
            Key::CtrlKey('y'),
            Key::CtrlKey('z'),
            Key::Letter(' '),
            Key::Letter('!'),
            Key::Letter('"'),
            Key::Letter('#'),
            Key::Letter('$'),
            Key::Letter('%'),
            Key::Letter('&'),
            Key::Letter('('),
            Key::Letter(')'),
            Key::Letter('*'),
            Key::Letter('+'),
            Key::Letter(','),
            Key::Letter('-'),
            Key::Letter('.'),
            Key::Letter('/'),
            Key::Letter('0'),
            Key::Letter('1'),
            Key::Letter('2'),
            Key::Letter('3'),
            Key::Letter('4'),
            Key::Letter('6'),
            Key::Letter('7'),
            Key::Letter('8'),
            Key::Letter('9'),
            Key::Letter(':'),
            Key::Letter(';'),
            Key::Letter('<'),
            Key::Letter('='),
            Key::Letter('>'),
            Key::Letter('?'),
            Key::Letter('@'),
            Key::Letter('A'),
            Key::Letter('B'),
            Key::Letter('C'),
            Key::Letter('D'),
            Key::Letter('E'),
            Key::Letter('F'),
            Key::Letter('H'),
            Key::Letter('I'),
            Key::Letter('J'),
            Key::Letter('K'),
            Key::Letter('L'),
            Key::Letter('M'),
            Key::Letter('N'),
            Key::Letter('O'),
            Key::Letter('P'),
            Key::Letter('Q'),
            Key::Letter('R'),
            Key::Letter('S'),
            Key::Letter('T'),
            Key::Letter('V'),
            Key::Letter('W'),
            Key::Letter('X'),
            Key::Letter('Y'),
            Key::Letter('Z'),
            Key::Letter('['),
            Key::Letter('\\'),
            Key::Letter(']'),
            Key::Letter('^'),
            Key::Letter('_'),
            Key::Letter('`'),
            Key::Letter('a'),
            Key::Letter('b'),
            Key::Letter('c'),
            Key::Letter('d'),
            Key::Letter('e'),
            Key::Letter('f'),
            Key::Letter('h'),
            Key::Letter('i'),
            Key::Letter('j'),
            Key::Letter('k'),
            Key::Letter('l'),
            Key::Letter('m'),
            Key::Letter('n'),
            Key::Letter('o'),
            Key::Letter('p'),
            Key::Letter('q'),
            Key::Letter('r'),
            Key::Letter('s'),
            Key::Letter('t'),
            Key::Letter('v'),
            Key::Letter('w'),
            Key::Letter('x'),
            Key::Letter('y'),
            Key::Letter('z'),
            Key::Letter('{'),
            Key::Letter('|'),
            Key::Letter('}'),
            Key::Letter('~'),
            Key::Delete,
        ]
        .into_iter();
        for seq in simulated_keys {
            let reader = std::io::Cursor::new(seq);
            let mut iter = reader.bytes();
            let key = KeyEvent::parse_key(iter.next().unwrap().unwrap(), &mut iter).unwrap();
            assert_eq!(key, expected.next().unwrap());
        }
    }
}
