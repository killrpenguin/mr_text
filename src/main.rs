// #![allow(unused_imports, unused_variables)]
// #![allow(dead_code)]

extern crate libc;

use mr_text::ffi::*;
use ropey::Rope;
use std::{
    io::{BufRead, ErrorKind, Read, Write},
    //    os::{fd::AsRawFd, unix::ffi::OsStrExt},
};

fn main() {
    let mut screen = Screen::new()
        .text_window()
        .mode_line()
        .left_margin()
        .backup_terminal()
        .build();

    Screen::write_text(&mut screen.io.ostream, &screen.left_margin.seperator_line);
    Screen::write_text(&mut screen.io.ostream, &screen.mode_line.seperator_line);
    Screen::write_text(&mut screen.io.ostream, &screen.mode_line.greeting);

    match screen.move_cursor(1, 3) {
        Ok(()) => {}
        Err(err) => eprintln!("{}", err),
    }
    match ffi::configure_raw(&mut screen.io.istream) {
        Ok(()) => {}
        Err(err) => panic!("{}", err),
    }
    loop {
        match screen.io.ascii_strategy() {
            Some(()) => {}
            None => break,
        }
        match screen.update_point() {
            Ok(()) => {}
            Err(err) => eprintln!("{}", err),
        };
        if !screen.mode_line.echo_area.message.is_empty() {
            screen.mode_line.echo_area.check_msg_timer();
        }
        match screen.write_mode_line() {
            Ok(()) => {}
            Err(err) => eprintln!("{}", err),
        }
    }
    match IO::write_esc_seq(&mut screen.io.ostream, EscSeq::ClrScrn(CLR_SCRN)) {
        Ok(()) => {}
        Err(err) => eprintln!("{}", err),
    }

    let _revert_on_drop =
        ffi::RevertOnDrop::new(screen.io.istream.by_ref(), screen.io.original_term.unwrap());
}

enum EscSeq<'a> {
    MvUp(&'a str),
    MvDown(&'a str),
    MvLeft(&'a str),
    MvRight(&'a str),
    ClrScrn(&'a str),
    ClrPntFwd(&'a str),
}

#[derive(Debug)]
struct Screen {
    io: IO,
    text_window: TextWindow,
    mode_line: ModeLine,
    left_margin: LeftMargin,
    point: Point,
}

pub trait Builder {
    fn text_window(self) -> Self;
    fn mode_line(self) -> Self;
    fn left_margin(self) -> Self;
    fn point(self) -> Self;
    fn backup_terminal(self) -> Self;
    fn build(self) -> Self;
}

impl Builder for Screen {
    fn build(self) -> Self {
        Screen {
            io: self.io,
            text_window: self.text_window,
            mode_line: self.mode_line,
            left_margin: self.left_margin,
            point: self.point,
        }
    }

    fn text_window(mut self) -> Self {
        let mut istream = std::io::stdin();
        let winsize = match ffi::io_ctl(istream.by_ref()) {
            Ok(winsize) => winsize,
            Err(err) => panic!("Possible uninitialized memory. \nError: {}", err),
        };
        self.text_window = TextWindow {
            winsize_row: winsize.ws_row,
            winsize_col: winsize.ws_col,
        };
        self
    }

    fn mode_line(mut self) -> Self {
        let mut bite_buf = [0; 4];
        let sep_as_str = '='.encode_utf8(&mut bite_buf);
        for _ in 0..self.text_window.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }

        self.mode_line = ModeLine {
            seperator: '=',
            thickness: 3,
            seperator_line: self.mode_line.seperator_line,
            greeting: "Hi David!".to_string(),
            echo_area: EchoArea::default(),
        };
        self
    }

    fn left_margin(mut self) -> Self {
        let mut new_line = String::new();
        for _ in 0..self.text_window.winsize_row {
            new_line.push_str("~\n\r");
        }

        self.left_margin = LeftMargin {
            seperator: '~',
            thickness: 2,
            seperator_line: new_line,
        };
        self
    }

    fn point(self) -> Self {
        Point { row: 0, col: 0 };
        self
    }

    fn backup_terminal(mut self) -> Self {
        self.io.original_term = match ffi::tc_getattr(&mut self.io.istream) {
            Ok(backup) => Some(backup),
            Err(err) => panic!("Error: {}", err),
        };
        self
    }
}

impl Screen {
    pub fn new() -> Screen {
        Screen {
            io: IO::new(),
            text_window: TextWindow::default(),
            mode_line: ModeLine::default(),
            left_margin: LeftMargin::default(),
            point: Point::default(),
        }
    }

    pub fn move_cursor(&mut self, row: u16, col: u16) -> std::io::Result<()> {
        write!(self.io.ostream, "\x1b[{};{}H", row, col)?;
        self.io.ostream.flush()?;
        Ok(())
    }

    pub fn write_text<S>(stream: &mut S, text: &str)
    where
        S: Write + ?Sized,
    {
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.flush();
    }

    pub fn write_mode_line(&mut self) -> std::io::Result<()> {
        write!(
            self.io.ostream,
            "\x1b[{};{}H",
            self.text_window.winsize_row, 0
        )?;
        IO::write_esc_seq(&mut self.io.ostream, EscSeq::ClrPntFwd(CLR_LN_CURSR_END))?;
        write!(
            self.io.ostream,
            "R: {}, C: {}",
            self.point.row, self.point.col
        )?;
        write!(
            self.io.ostream,
            "\x1b[{};{}H",
            self.text_window.winsize_row, 24
        )?;
        write!(self.io.ostream, "{}", self.mode_line.echo_area)?;
        write!(
            self.io.ostream,
            "\x1b[{};{}H",
            self.point.row, self.point.col
        )?;
        self.io.ostream.flush()?;
        Ok(())
    }

    pub fn update_point(&mut self) -> std::io::Result<()> {
        let mut lock = self.io.istream.lock();
        write!(self.io.ostream, "{}", REQ_CURSOR_POS)?;
        self.io.ostream.flush()?;

        let buf = match lock.fill_buf() {
            Ok(buf) if buf[..].len() > ESC_SEQ_LEN => buf,
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                return Ok(());
            }
            error => {
                write!(self.io.estream, "{:?}", error)?;
                return Ok(());
            }
        };

        let len = buf[..].len();
        let semicolon = match buf.into_iter().position(|chr| *chr == 59) {
            Some(idx) => idx,
            None => {
                lock.consume(len);
                return Ok(());
            }
        };
        let trim = (semicolon + 1, len - 1);
        if let Some(row) = from_utf8_escape_seq(buf, 2 as usize, semicolon) {
            self.point.row = match row.parse::<u16>() {
                Ok(num) => num,
                Err(err) => {
                    write!(self.io.estream, "{}", err)?;
                    lock.consume(len);
                    return Ok(());
                }
            };
        }
        if let Some(col) = from_utf8_escape_seq(buf, trim.0, trim.1) {
            self.point.col = match col.parse::<u16>() {
                Ok(num) => num,
                Err(err) => {
                    write!(self.io.estream, "{}", err)?;
                    lock.consume(len);
                    return Ok(());
                }
            };
        }
        lock.consume(len);
        Ok(())
    }
}

#[derive(Debug)]
struct IO {
    istream: std::io::Stdin,
    ostream: std::io::Stdout,
    estream: std::io::Stderr,
    rope: ropey::Rope,
    read_len: usize,
    original_term: Option<libc::termios>,
}

impl Default for IO {
    fn default() -> Self {
        IO::new()
    }
}

impl IO {
    pub fn new() -> IO {
        IO {
            istream: std::io::stdin(),
            ostream: std::io::stdout(),
            estream: std::io::stderr(),
            rope: Rope::new(),
            read_len: 0,
            original_term: None,
        }
    }

    fn write_esc_seq<'a, T>(stream: &mut T, key_binding: EscSeq<'a>) -> std::io::Result<()>
    where
        T: Write + ?Sized,
    {
        let action = match key_binding {
            EscSeq::MvDown(seq) => seq,
            EscSeq::MvUp(seq) => seq,
            EscSeq::MvLeft(seq) => seq,
            EscSeq::MvRight(seq) => seq,
            EscSeq::ClrScrn(seq) => seq,
            EscSeq::ClrPntFwd(seq) => seq,
        };
        write!(stream, "{}", action)?;
        stream.flush()?;
        Ok(())
    }

    // pub fn ctrl_keys_strategy<T>(
    //     ostream: &mut T,
    //     buffer: &[u8],
    //     rope: &mut ropey::Rope,
    // ) -> Option<()>
    // where
    //     T: Write + ?Sized,
    // {
    //     match buffer {
    //         [2] => {
    //             IO::write_esc_seq(&mut *ostream, EscSeq::MvLeft(MV_LEFT)).unwrap();
    //         }
    //         [4] => {} // test key  TEST_KEY
    //         [6] => {
    //             IO::write_esc_seq(&mut *ostream, EscSeq::MvRight(MV_RIGHT)).unwrap();
    //         }
    //         [9] => {
    //             Screen::write_text(&mut *ostream, "\t");
    //             rope.insert(rope.len_chars(), "\t");
    //         }
    //         [10] => {
    //             Screen::write_text(&mut *ostream, "\n\r~ ");
    //             rope.insert(rope.len_chars(), "\n");
    //         }
    //         [14] => {
    //             IO::write_esc_seq(&mut *ostream, EscSeq::MvUp(MV_UP)).unwrap();
    //         }
    //         [16] => {
    //             IO::write_esc_seq(&mut *ostream, EscSeq::MvDown(MV_DOWN)).unwrap();
    //         }
    //         [17] => {
    //             return None;
    //         }
    //         _ => eprintln!("Key unimplemented: {:?}", buffer),
    //     }
    //     Some(())
    // }

    pub fn ctrl_keys_strategy(&mut self, buffer: &[u8]) -> Option<()> {
        match buffer {
            [2] => {
                IO::write_esc_seq(&mut self.ostream, EscSeq::MvLeft(MV_LEFT)).unwrap();
            }
            [4] => {} // test key  TEST_KEY
            [6] => {
                IO::write_esc_seq(&mut self.ostream, EscSeq::MvRight(MV_RIGHT)).unwrap();
            }
            [9] => {
                Screen::write_text(&mut self.ostream, "\t");
                self.rope.insert(self.rope.len_chars(), "\t");
            }
            [10] => {
                Screen::write_text(&mut self.ostream, "\n\r~ ");
                self.rope.insert(self.rope.len_chars(), "\n");
            }
            [14] => {
                IO::write_esc_seq(&mut self.ostream, EscSeq::MvUp(MV_UP)).unwrap();
            }
            [16] => {
                IO::write_esc_seq(&mut self.ostream, EscSeq::MvDown(MV_DOWN)).unwrap();
            }
            [17] => {
                return None;
            }
            _ => eprintln!("Key unimplemented: {:?}", buffer),
        }
        Some(())
    }

    pub fn ascii_strategy(&mut self) -> Option<()> {
        let mut lock = self.istream.lock();
        let buffer = match lock.fill_buf() {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::Interrupted => lock.fill_buf().unwrap(),
            error => error.unwrap(),
        };

        let read_len = buffer.len();
        match buffer {
            [0..=31] => match self.ctrl_keys_strategy(&*buffer) {
                Some(()) => {
                    lock.consume(read_len);
                    return Some(());
                }
                None => {
                    lock.consume(read_len);
                    return None;
                }
            },
            [32..=126] => {
                let mut input = match std::str::from_utf8(buffer) {
                    Ok(input) => input,
                    Err(error) => match error.error_len() {
                        // TODO: What to do with after_valid?
                        Some(_len) => {
                            let (valid, _after_valid) = buffer.split_at(error.valid_up_to());
                            if valid.len() >= 1 {
                                unsafe { std::str::from_utf8_unchecked(valid) }
                            } else {
                                return Some(());
                            }
                        }
                        None => return Some(()), // char could be valid once utf8 is better implemented.
                    },
                };
                self.rope.insert(self.rope.len_chars(), input);
                Screen::write_text(&mut self.ostream, &mut input);
            }
            [127] => {
                IO::write_esc_seq(&mut self.ostream, EscSeq::MvLeft(MV_LEFT)).unwrap();
                IO::write_esc_seq(&mut self.ostream, EscSeq::ClrPntFwd(CLR_LN_CURSR_END)).unwrap();
            }
            _ if buffer.len() > 1 => match buffer {
                [27, 120] => println!("Hi"),
                [27, 91, 68] => {
                    IO::write_esc_seq(&mut self.ostream, EscSeq::MvLeft(MV_LEFT)).unwrap()
                }
                [27, 91, 67] => {
                    IO::write_esc_seq(&mut self.ostream, EscSeq::MvRight(MV_RIGHT)).unwrap()
                }
                [27, 91, 66] => IO::write_esc_seq(&mut self.ostream, EscSeq::MvUp(MV_UP)).unwrap(),
                [27, 91, 65] => {
                    IO::write_esc_seq(&mut self.ostream, EscSeq::MvDown(MV_DOWN)).unwrap()
                }
                _ => eprintln!("Key unimplemented: {:?}", buffer),
            },
            _ => {
                eprintln!("Key unimplemented: {:?}", buffer);
            }
        }

        lock.consume(read_len);
        Some(())
    }
}

/// Use the screen builder method to construct Point. This private struct is used
/// inside ModeLine.
#[derive(Debug, Default, Clone)]
struct Point {
    row: u16,
    col: u16,
}

/// Use the screen builder method to construct a TextWindow. It is important that the builder
/// method be used first in any screen builder sequence.
///
/// # Examples
///
/// ```
///
/// let mut Screen = Screen::new();
///
/// let mut screen = Screen::new()
///     .text_window()
///     .mode_line()
///     .left_margin()
///     .backup_terminal()
///     .build();
///
/// ``
#[derive(Debug, Default, Clone, Copy)]
struct TextWindow {
    winsize_row: u16,
    winsize_col: u16,
}
impl TextWindow {}

/// Use the screen builder method to constructs a ModeLine struct. The initial field value of
/// ModeLine thickness is 3 and the initial seperator is '='. Example shows how to change these settings.
///
/// Use Screen.new_ml_sep_line() to refresh the seporator_line field and apply changes.
/// # Examples
///
/// ```
///
/// let mut mode_line = ModeLine::default();
///
/// mode_line.new_set('a');
/// mode_line.new_thickness(10);
///
/// assert_eq!(mode_line.seperator, 'a');
/// assert_eq!(left_margin.thickness, 10);
/// ``
#[derive(Debug, Default, Clone)]
pub struct ModeLine {
    thickness: u16,
    seperator: char,
    seperator_line: String,
    greeting: String,
    echo_area: EchoArea,
}

impl ModeLine {
    pub fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    pub fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }

    pub fn refresh_sep_line(&mut self) {
        todo!()
    }
}

/// Use the screen builder method to constructs a ModeLine struct. The disp_len field adjusts the amout of time
/// the message will be displayed in the mode line.
///
/// # Examples
///
/// ```
///
/// let mut message = message::default();
///
/// left_margin.new_set('a');
/// left_margin.new_thickness(10);
///
/// assert_eq!(left_margin.seperator, 'a');
/// assert_eq!(left_margin.thickness, 10);
/// ``
#[derive(Debug, Clone)]
struct EchoArea {
    message: String,
    msg_timer: std::time::Instant,
    disp_len: u64,
}

impl std::fmt::Display for EchoArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Default for EchoArea {
    fn default() -> Self {
        EchoArea {
            message: std::string::String::with_capacity(64),
            msg_timer: std::time::Instant::now(),
            disp_len: 10,
        }
    }
}
impl EchoArea {
    pub fn new_disp_len(&mut self, len: u64) {
        self.disp_len = len
    }

    pub fn check_msg_timer(&mut self) {
        if self.msg_timer.elapsed().as_secs() == self.disp_len {
            self.message.clear();
        }
    }

    pub fn store_msg<'a>(&mut self, msg: &'a str) {
        if msg.len() > self.message.capacity() {
            // Message field created with a capacity of 64.
            // TODO: Convert to deque data struct and remove capacity.
            self.message.push_str(&msg[..64]);
        } else {
            self.message.push_str(msg);
        }
        self.msg_timer = std::time::Instant::now();
    }
}

/// Use the Screen builder method to construct a new LeftMargin. The thickness field is measured in
/// terminal rows. Each row is the thickness of one character. The initial field value of LeftMargin
/// thickness is 2, meaning two rows or about two characters. The initial seperator is '~'.
/// Example shows how to change these settings. Use the screen builder method a second time to reconstruct
/// the seperator line.
///
/// # Examples
///
/// ```
/// let mut screen_ex = Screen::default().left_margin();
///
/// screen_ex.left_margin.new_sep('a');
/// screen_ex.left_margin.new_thickness(10);
/// screen_ex.left_margin.refresh_sep_line();
///
/// assert_eq!(left_margin.seperator, 'a');
/// assert_eq!(left_margin.thickness, 10);
/// ``
#[derive(Debug, Default, Clone)]
struct LeftMargin {
    thickness: u16,
    seperator: char,
    seperator_line: String,
}

impl LeftMargin {
    pub fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    pub fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }

    pub fn refresh_sep_line(&mut self) {
        todo!()
    }
}

///
/// # Safety
/// If part of the buf argument in returns invalid this function will use the unsafe from_utf8_unchecked
/// on the valid portion of the code to continue the conversion. The valid subslice is
/// split before the error index and and has already been validated.
///
///
/// # Examples
///
/// Basic usage:
///
/// ```
///
/// let valid_buf: &[u8] = &[27, 91, 50, 51, 50, 59, 51, 49, 52, 82];
/// let valid = from_utf8_escape(valid_buf, 0 as usize, buf.len());
///
/// let invalid_buf: &[u8] = &[27, 91, 50, 4, 59, 51, 82];
/// let invalid = from_utf8_escape(invalid_buf, 0 as usize, buf.len());
///
/// assert_eq!(valid, Some("\u{1b}[232;314R"))
/// assert_eq!(invalid, None)) //
///
///
pub fn from_utf8_escape_seq<'a>(buf: &'a [u8], start: usize, end: usize) -> Option<&'a str> {
    match std::str::from_utf8(&buf[start..end]) {
        Ok(row) => Some(row),
        Err(error) => {
            // TODO: Again. What to do with after_valid?
            let (valid, _after_valid) = buf.split_at(error.valid_up_to());
            if valid.len() > ESC_SEQ_LEN {
                return Some(unsafe { std::str::from_utf8_unchecked(valid) });
            } else {
                return None;
            }
        }
    }
}

/// Readability constants.
const CLR_SCRN: &str = "\x1b[2J";
const CLR_LN_CURSR_END: &str = "\x1b[0K";
const CLR_LN: &str = "\x1b[2k";

const REQ_CURSOR_POS: &str = "\x1b[6n";

const MV_LEFT: &str = "\x1b[1D";
const MV_RIGHT: &str = "\x1b[1C";
const MV_UP: &str = "\x1b[1B";
const MV_DOWN: &str = "\x1b[1A";

const ESC_SEQ_LEN: usize = 5;
const UTF8_SIZE: usize = 4;

#[cfg(test)]
mod tests {
    //    use super::*;
}
