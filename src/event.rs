#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

use std::io::{BufRead, Read};

pub struct KeyEvent {}

impl KeyEvent {
    fn parse_char<R>(reader: &mut R) -> std::io::Result<Option<Key>>
    where
        R: BufRead,
    {
        let buffer = match reader.fill_buf() {
            Ok(buf) => buf,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };

        if buffer.len() == 1 {
            let ch = buffer.iter().next().unwrap();
            if ch.is_ascii() {
                return Ok(Some(Key::Ascii(*ch as char)));
            } else {
                return Err(std::io::Error::last_os_error());
            }
        }
        let error = std::io::Error::new(std::io::ErrorKind::Other, "Invalid Utf8");
        let bytes = &mut Vec::new();
        let mut iter = buffer.iter();
        loop {
            match iter.next() {
                Some(byte) => {
                    bytes.push(*byte);
                    if let Ok(s) = std::str::from_utf8(bytes) {
                        return Ok(Some(Key::Utf8(s.chars().next().unwrap())));
                    }
                    if bytes.len() >= 4 {
                        return Err(error);
                    }
                }
                _ => return Ok(None),
            }
        }
    }

    fn parse_cursor_pos<R>(reader: &mut R) -> std::io::Result<Option<Key>>
    where
        R: BufRead,
    {
        let mut pos: Vec<u8> = vec![];
        let mut ret_val = (0, 0);
        let buffer = match reader.fill_buf() {
            Ok(buf) => buf,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        let mut iter = buffer.iter();
        loop {
            match iter.next() {
                Some(byte) => {
                    if byte.is_ascii_digit() {
                        pos.push(*byte);
                    }
                    if *byte == b';' {
                        ret_val.0 = pos.iter().fold(0, |acc, c| acc * 10 + (c - b'0') as u16);

                        pos.clear();
                    }
                }
                _ => break,
            }
        }
        ret_val.1 = pos.iter().fold(0, |acc, c| acc * 10 + (c - b'0') as u16);
        Ok(Some(Key::CursorPos(ret_val)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Ascii(char),
    Utf8(char),
    CursorPos((u16, u16)),
    CtrlKey(char),
    AltKey(char),
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    Delete,
    Insert,
    F(u8),
    Esc,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_chars() {
        let st = "tE7!éŷ¤£€ù";
        let mut expected = vec![
            Key::Ascii('t'),
            Key::Ascii('E'),
            Key::Ascii('7'),
            Key::Ascii('!'),
            Key::Utf8('é'),
            Key::Utf8('ŷ'),
            Key::Utf8('¤'),
            Key::Utf8('£'),
            Key::Utf8('€'),
            Key::Utf8('ù'),
        ]
        .into_iter();
        for byte in st.chars() {
            let b = format!("{}", byte);
            let mut reader = std::io::Cursor::new(b);
            assert_eq!(
                KeyEvent::parse_char(&mut reader).unwrap(),
                Some(expected.next().unwrap())
            );
        }
    }
    #[test]
    fn test_parse_cursor_pos() {
        let st = "\x1b[23;23R";
        let mut reader = std::io::Cursor::new(st);
        assert_eq!(
            KeyEvent::parse_cursor_pos(&mut reader).unwrap(),
            Some(Key::CursorPos((23, 23)))
        )
    }
}
