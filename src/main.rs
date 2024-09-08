#![allow(unused_imports, unused_variables, unused_mut)]
#![allow(dead_code)]

extern crate libc;

use mr_text::ffi::*;
use ropey::RopeBuilder;
use std::{
    borrow::BorrowMut,
    io::{BufRead, BufWriter, Error, Read, Stdin, Write},
    os::{fd::AsRawFd, unix::ffi::OsStrExt},
};

const ESC_SEQ_CLR_LN: &str = "\x1b[K";

fn main() {
    let mut screen = Screen::new()
        .text_window()
        .mode_line()
        .left_margin()
        .backup_terminal()
        .build();

    Screen::draw(&mut screen.io.ostream, &screen.left_margin.seperator_line);
    Screen::draw(&mut screen.io.ostream, &screen.mode_line.seperator_line);
    Screen::draw(&mut screen.io.ostream, &screen.mode_line.cursor_pos);

    screen.cursor_to_home(1, 3);
    ffi::configure_raw(&mut screen.io.istream).unwrap();

    loop {
        match IO::parse_key(
            &mut screen.io.istream,
            &mut screen.io.ostream,
            &mut screen.io.rope,
        ) {
            Some(()) => {}
            None => break,
        }
        screen.update_point();
        screen.draw_point_pos();
    }
    screen.clear();
    let _revert_on_drop =
        ffi::RevertOnDrop::new(screen.io.istream.by_ref(), screen.io.original_term.unwrap());
}

#[derive(Debug)]
pub struct Screen {
    io: IO,
    text_window: TextWindow,
    mode_line: ModeLine,
    tab_line: TabLine,
    left_margin: LeftMargin,
}

pub trait Builder {
    fn text_window(self) -> Self;
    fn mode_line(self) -> Self;
    fn left_margin(self) -> Self;
    fn backup_terminal(self) -> Self;
    fn build(self) -> Self;
}

impl Builder for Screen {
    /// #Safety
    /// Saftey concern. Possible uninizialized memory in init_text_window().
    /// Panic on error.

    fn build(mut self) -> Self {
        Screen {
            io: self.io,
            text_window: self.text_window,
            mode_line: self.mode_line,
            tab_line: self.tab_line,
            left_margin: self.left_margin,
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
            text_area_row: winsize.ws_row - 3,
            text_area_col: winsize.ws_col - 2,
        };
        self
    }

    fn mode_line(mut self) -> Self {
        let mut bite_buf = [0; 4];
        let sep_as_str = '='.encode_utf8(&mut bite_buf);
        let mut new_line = String::new();
        for _ in 0..self.text_window.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }

        self.mode_line = ModeLine {
            seperator: '=',
            thickness: 3,
            seperator_line: self.mode_line.seperator_line,
            cursor_pos: "Hello World".to_string(),
            point: Point::default(),
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
            tab_line: TabLine::default(),
            left_margin: LeftMargin::default(),
        }
    }

    pub fn test(&mut self) {}

    pub fn cursor_to_home(&mut self, row: u16, col: u16) {
        self.mode_line.point.row = row;
        self.mode_line.point.col = col;
        let _ = write!(self.io.ostream, "\x1b[{};{}H", row, col);
        let _ = self.io.ostream.flush();
    }

    pub fn clear(&mut self) {
        let _ = write!(self.io.ostream, "{}[2J", 27 as char);
        let _ = self.io.ostream.flush();
    }

    pub fn clear_line(&mut self) {
        let _ = write!(self.io.ostream, "{}", ESC_SEQ_CLR_LN);
        let _ = self.io.ostream.flush();
    }

    pub fn draw<S>(stream: &mut S, text: &str)
    where
        S: Write + ?Sized,
    {
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.flush();
    }

    pub fn draw_point_pos(&mut self) {
        let _ = write!(
            self.io.ostream,
            "\x1b[{};{}H",
            self.text_window.winsize_row, 0
        );
        let _ = self.io.ostream.flush();
        self.clear_line();
        let _ = write!(
            self.io.ostream,
            "R: {}, C: {}",
            self.mode_line.point.row, self.mode_line.point.col
        );
        let _ = self.io.ostream.flush();
        let _ = write!(
            self.io.ostream,
            "\x1b[{};{}H",
            self.mode_line.point.row, self.mode_line.point.col
        );
        let _ = self.io.ostream.flush();
    }

    pub fn keybinding_movement<T>(stream: &mut T, dir: char, lines: Option<u8>)
    where
        T: Write + ?Sized,
    {
        let _ = write!(stream, "\x1b[{}{}", lines.unwrap_or(1), dir);
        let _ = stream.flush();
    }

    pub fn update_point(&mut self) {
        let _ = self.io.ostream.write_all(b"\x1b[6n");
        let _ = self.io.ostream.flush();
        let mut lock = self.io.istream.lock();
        let buf = lock.fill_buf().expect("Buffer empty.");
        let semicolon = buf
            .into_iter()
            .position(|chr| *chr == 59)
            .expect("No semicolon found.");
        let len = buf.len();
        let trim = (semicolon + 1, len - 1);
        self.mode_line.point.row = std::str::from_utf8(&buf[2..semicolon])
            .unwrap_or("1234")
            .parse::<u16>()
            .expect("Parse to u16 failed. ");
        self.mode_line.point.col = std::str::from_utf8(&buf[trim.0..trim.1])
            .unwrap_or("1234")
            .parse::<u16>()
            .expect("Parse to u16 failed. ");
        lock.consume(len);
    }
}

#[derive(Debug)]
pub struct IO {
    istream: std::io::Stdin,
    ostream: std::io::Stdout,
    estream: std::io::Stderr,
    rope: ropey::RopeBuilder,
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
            rope: RopeBuilder::new(),
            read_len: 0,
            original_term: None,
        }
    }

    pub fn parse_key<T>(
        istream: &mut std::io::Stdin,
        ostream: &mut T,
        rope: &mut ropey::RopeBuilder,
    ) -> Option<()>
    where
        T: Write + ?Sized,
    {
        let mut lock = istream.lock();
        let mut buffer = lock.fill_buf().expect("istream buffer empty.");
        let read_len = buffer.len();

        match buffer.into_iter().next().expect("istream buffer empty.") {
            // 8 is backspace
            2 => {
                Screen::keybinding_movement(&mut *ostream, 'D', None);
            } // ctrl + b
            4 => {
                // test key
            }
            6 => {
                Screen::keybinding_movement(&mut *ostream, 'C', None);
            } // ctrl + f
            9 => {
                Screen::draw(&mut *ostream, "\t");
                rope.append("\t");
            }
            10 => {
                Screen::draw(&mut *ostream, "\r\n~ ");
                rope.append("\n");
            }
            14 => {
                Screen::keybinding_movement(&mut *ostream, 'B', None);
            } // ctrl + p
            16 => {
                Screen::keybinding_movement(&mut *ostream, 'A', None);
            } // ctrl + n
            17 => return None,
            32..=126 => {
                let mut input = std::str::from_utf8(buffer).expect("Invalid utf8.");
                rope.append(input);
                Screen::draw(&mut *ostream, &mut input);
            }
            // 127 is del
            _ => {
                println!("Key unimplemented: {:?}", buffer);
            }
        };
        lock.consume(read_len);
        Some(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TextWindow {
    winsize_row: u16,
    winsize_col: u16,
    text_area_row: u16,
    text_area_col: u16,
}
impl TextWindow {}

/// Constructs a ModeLine struct. The initial field value of ModeLine
/// thickness is 3 and the initial seperator is '='. Example shows how to change these settings.
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
pub struct Point {
    row: u16,
    col: u16,
}

#[derive(Debug, Default, Clone)]
pub struct ModeLine {
    thickness: u16,
    seperator: char,
    seperator_line: String,
    cursor_pos: String,
    point: Point,
}

impl ModeLine {
    pub fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    pub fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }
}

/// Constructs a new LeftMargin struct. The initial field value of LeftMargin
/// thickness is 2 and the initial seperator is '~'. Example shows how to change these settings.
///
/// # Examples
///
/// ```
///
/// let mut left_margin = LeftMargin::default();
///
/// left_margin.new_set('a');
/// left_margin.new_thickness(10);
///
/// assert_eq!(left_margin.seperator, 'a');
/// assert_eq!(left_margin.thickness, 10);
/// ``

#[derive(Debug, Default, Clone)]
pub struct LeftMargin {
    thickness: u16,
    seperator: char,
    seperator_line: String,
}

impl LeftMargin {
    fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }
}

#[derive(Debug, Default, Clone)]
pub struct TabLine {}
impl TabLine {}
