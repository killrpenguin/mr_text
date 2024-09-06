#![allow(unused_imports, unused_variables, unused_mut)]
#![allow(dead_code)]

extern crate libc;

use mr_text::ffi::*;
use std::{
    borrow::BorrowMut,
    io::{BufRead, Read, Write},
};

fn main() {
    let mut screen = Screen::new()
        .init_left_margin()
        .init_mode_line()
        .init_text_window()
        .backup_terminal();

    screen.clear();
    screen.position_cursor("1", "3");
    Screen::draw(&mut screen.io.ostream, &screen.mode_line.seperator_line);
    Screen::draw(&mut screen.io.ostream, &screen.left_margin.seperator_line);

    ffi::configure_raw(&mut screen.io.istream).unwrap();

    loop {
        break;
    }
    screen.clear();
    let _revert_on_drop = ffi::RevertOnDrop::new(screen.io.istream.by_ref(), screen.io.original_term);
}

pub struct Screen<'a> {
    io: IO,
    point: Point<'a>,
    text_window: TextWindow,
    mode_line: ModeLine,
    tab_line: TabLine,
    left_margin: LeftMargin,
}

pub trait Builder<'a> {
    fn init_text_window(self) -> Screen<'a>;
    fn init_mode_line(self) -> Screen<'a>;
    fn new_ml_sep_line(&mut self);
    fn init_left_margin(self) -> Screen<'a>;
    fn backup_terminal(self) -> Screen<'a>;
}

impl<'a> Builder<'a> for Screen<'a> {
    /// init_text_window has to be built last so that the thickness of the lines
    /// and margins have been populated.
    ///
    /// Saftey concern. Possible uninizialized memory in init_text_window().
    /// Panic on error.

    fn init_text_window(mut self) -> Screen<'a> {
        let mut istream = std::io::stdin();
        let winsize = match ffi::io_ctl(istream.by_ref()) {
            Ok(winsize) => winsize,
            Err(err) => panic!("Possible uninitialized memory. \nError: {}", err),
        };
        self.text_window = TextWindow {
            winsize_row: winsize.ws_row,
            winsize_col: winsize.ws_col,
            text_area_row: winsize.ws_row - self.mode_line.thickness,
            text_area_col: winsize.ws_col - self.left_margin.thickness,
        };
        self
    }

    fn init_left_margin(mut self) -> Screen<'a> {
        self.left_margin.new_sep('~');
        self.left_margin.new_thickness(2);

        let sep_str = format!("{}\n\r", self.left_margin.seperator);
        for _ in 0..self.text_window.winsize_row {
            self.left_margin.seporator_line.push_str(&sep_str);
        }
        self
    }

    fn init_mode_line(mut self) -> Screen<'a> {
        self.mode_line.new_sep('=');
        self.mode_line.new_thickness(3);

        let mut bite_buf = [0; 4];
        let sep_as_str = self.mode_line.seperator.encode_utf8(&mut bite_buf);
        for _ in 0..self.text_window.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }
        self
    }

    fn new_ml_sep_line(&mut self) {
        let mut bite_buf = [0; 4];
        let sep_as_str = self.mode_line.seperator.encode_utf8(&mut bite_buf);
        // Only reallocates if winsize.winsize_col has increased.
        self.mode_line.seperator_line.clear();
        for _ in 0..self.text_window.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }
        // TODO: Add draw method here when I have one written.
    }

    fn backup_terminal(mut self) -> Screen<'a> {
        self.io.original_term = match ffi::tc_getattr(&mut self.io.istream) {
            Ok(backup) => Some(backup),
            Err(err) => panic!("Error: {}", err),
        };
        self
    }
}

impl<'a> Screen<'a> {
    pub fn new() -> Screen<'a> {
        Screen {
            io: IO::new(),
            point: Point::default(),
            text_window: TextWindow::default(),
            mode_line: ModeLine::default(),
            tab_line: TabLine::default(),
            left_margin: LeftMargin::default(),
        }
    }
    pub fn position_cursor(&mut self, row: &'a str, col: &'a str) {
        self.point.row = row;
        self.point.col = col;
        write!(self.io.ostream, "\x1b[{};{}H", row, col).unwrap();
        self.io.ostream.flush().unwrap();
    }

    pub fn clear(&mut self) {
        write!(self.io.ostream, "{}[2J", 27 as char).expect("Write failed in clear_screen().");
        self.io.ostream.flush().expect("Failed post write flush.");
    }

    //         .write_all("\r\n~ ".as_bytes())

    pub fn draw<T>(stream: &mut T, text: &str)
    where
        T: Write + ?Sized,
    {
        stream
            .write_all(text.as_bytes())
            .expect("Failed to write buffer to ostream.");
        stream.flush().unwrap();
    }

    pub fn write_key_stroke(&mut self) {
        let start = self.io.buffer.len() - self.io.buf_len;
        self.io
            .ostream
            .write_all(&self.io.buffer[start..])
            .expect("Failed to write buffer to ostream.");
        self.io.ostream.flush().expect("Failed post write flush.");
        self.io.istream.lock().consume(self.io.buf_len);
    }

    pub fn rebuild_ml_seperator_line(&mut self) {
        let mut bite_buf = [0; 4];
        let sep_as_str = self.mode_line.seperator.encode_utf8(&mut bite_buf);
        for _ in 0..self.text_window.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }
    }
}

#[derive(Debug)]
pub struct IO {
    istream: std::io::Stdin,
    ostream: std::io::Stdout,
    estream: std::io::Stderr,
    buffer: Vec<u8>,
    buf_len: usize,
    original_term: Option<libc::termios>,
}
impl IO {
    pub fn new() -> IO {
        IO {
            istream: std::io::stdin(),
            ostream: std::io::stdout(),
            estream: std::io::stderr(),
            buffer: Vec::with_capacity(512), // temp data struct
            buf_len: 0,
            original_term: None,
        }
    }

    pub fn store_buffer<T: std::iter::IntoIterator<Item = u8> + ExactSizeIterator>(
        &mut self,
        buf: T,
    ) {
        self.buf_len = buf.len();
        self.buffer = buf.into_iter().collect::<Vec<_>>()
    }

    pub fn write_esc_seq(&mut self, escape_seq: &str) {
        write!(self.ostream, "{}", escape_seq).expect("Failed to write escape seq.");
        self.ostream.flush().unwrap();
    }
    pub fn read_esc_resp(&mut self) {
        todo!()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Point<'a> {
    row: &'a str,
    col: &'a str,
}
impl<'a> Point<'a> {
    pub fn update_pos(&mut self, row: &'a str, col: &'a str) {
        self.row = row;
        self.col = col;
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

/// Constructs a new mode line struct. The initial field value of ModeLine
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
pub struct ModeLine {
    thickness: u8,
    seperator: char,
    seperator_line: String,
}

impl ModeLine {
    pub fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    pub fn new_thickness(&mut self, thickness: u8) {
        self.thickness = thickness
    }
}

/// Constructs a new left margin struct. The initial field value of LeftMargin
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
    thickness: u8,
    seperator: char,
    seperator_line: String,
}

impl LeftMargin {
    fn new_sep(&mut self, sep: char) {
        self.seperator = sep
    }

    fn new_thickness(&mut self, thickness: u8) {
        self.thickness = thickness
    }
}

#[derive(Debug, Default, Clone)]
pub struct TabLine {}
impl TabLine {}
