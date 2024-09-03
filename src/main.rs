#![allow(unused_imports, unused_variables, unused_mut)]
#![allow(dead_code)]

extern crate libc;

use mr_text::ffi::*;
use std::io::{BufRead, Read, Write};

fn main() {
    let mut screen = Screen::new()
        .populate_winsize()
        .build_left_edge()
        .build_bottom_line();

    let original_term = match ffi::tc_getattr(&mut screen.terminal.istream) {
        Ok(backup) => backup,
        Err(err) => panic!("Error: {}", err),
    };

    screen.clear_screen();
    screen.render_edges();
    screen.position_cursor(1, 3);
    screen.render_mode_line();
    ffi::configure_raw(&mut screen.terminal.istream).unwrap();

    screen.update_point();
//    println!("{:?}{:?}", screen.point.t_c, screen.point.t_r);

    loop {
        screen
            .terminal
            .read_buffer()
            .expect("Failed to read from stdin.");
        let mut start = 0;
        let mut end = screen.terminal.buffer.len();
        let to_consume = screen.terminal.buffer.len();

        screen.terminal.validate_utf8(start, end);
        match screen.terminal.buffer.into_iter().next().unwrap() {
            // 8 is backspace
            2 => screen.keybinding_movement('D', None), // ctrl + b
            4 => {}
            6 => screen.keybinding_movement('C', None), // ctrl + f
            9 => screen.terminal.write_input(start, to_consume), // tab
            10 => {
                screen.terminal.cr_nl(to_consume);
            } // carriage return new line
            14 => screen.keybinding_movement('B', None), // ctrl + p
            16 => screen.keybinding_movement('A', None), // ctrl + n
            17 => {
                // Clear istream and quit.
                screen.terminal.istream.lock().consume(to_consume);
                break;
            }
            32..=126 => {
                screen.terminal.write_input(start, to_consume);
            }
            // 127 is del
            _ => {
                screen.terminal.istream.lock().consume(to_consume);
            }
        }
        screen.update_point();
        screen.render_mode_line();
    }
    screen.clear_screen();
    let _revert_on_drop = ffi::RevertOnDrop::new(screen.terminal.istream.by_ref(), original_term);
}
#[derive(Debug)]
pub struct Screen {
    terminal: Terminal,
    point: Point,
    left_edge: String,
    bottom_line: String,
    mode_line: String,
    winsize_row: u16,
    winsize_col: u16,
}

impl Screen {
    pub fn new() -> Self {
        Screen {
            terminal: Terminal::new(),
            point: Point::new(),
            left_edge: String::new(),
            bottom_line: String::new(),
            mode_line: " ".to_string(),
            winsize_row: 0,
            winsize_col: 0,
        }
    }

    pub fn populate_winsize(mut self) -> Screen {
        let mut istream = std::io::stdin();
        // Saftey concern. Possible uninizialized memory? Terminate on error.
        let winsize = match ffi::io_ctl(istream.by_ref()) {
            Ok(winsize) => winsize,
            Err(err) => panic!("{}", err),
        };
        self.winsize_row = winsize.ws_row;
        self.winsize_col = winsize.ws_col;
        self
    }

    /// This builder method must be called after populate_winsize().
    pub fn build_left_edge(mut self) -> Screen {
        let winsize_row = self.winsize_row - 2;
        for _ in 0..winsize_row {
            self.left_edge.push_str("~\n\r");
        }
        self
    }

    /// This builder method must be called after populate_winsize().
    pub fn build_bottom_line(mut self) -> Screen {
        for _ in 0..self.winsize_col {
            self.bottom_line.push_str("=");
        }
        self
    }

    pub fn get_point_pos(&mut self) {
        let mut lock = self.terminal.istream.lock();

        write!(self.terminal.ostream, "\x1b[6n").expect("Write failed in get_point_pos().");
        self.terminal.ostream.flush().expect("Failed to flush.");
        let output = lock.fill_buf().unwrap();
        let len = output.len();
        self.point.escape_buf = output.iter().map(|val| *val).collect::<Vec<u8>>();
        lock.consume(len);
    }

    pub fn update_point(&mut self) {
        self.get_point_pos();
        // trim the R
        let len = self.point.escape_buf.len() - 1;
        let escape_slice = &self.point.escape_buf[2..len];
        let semicolon_idx = escape_slice.iter().position(|&val| val == 59).unwrap();
        let row_col = escape_slice.split_at(semicolon_idx);
        let col = row_col.1.strip_prefix(&[59]).unwrap();
        self.point.r = row_col.0.iter().map(|val| *val - b'0').collect::<Vec<u8>>();
        self.point.row = (col[0] as u16) << 8;
        self.point.c = col.iter().map(|val| *val - b'0').collect::<Vec<u8>>();
    }

    pub fn position_cursor(&mut self, row: u16, col: u16) {
        self.point.row = row;
        self.point.col = col;
        write!(self.terminal.ostream, "\x1b[{};{}H", row, col).unwrap();
        self.terminal.ostream.flush().unwrap();
    }

    pub fn render_mode_line(&mut self) {
        let pos = (self.point.row, self.point.col);
        let mode_text = format!("R: {}, C: {}", self.point.row, self.point.col);
        self.position_cursor(self.winsize_col, 0);
        write!(self.terminal.ostream, "{}", mode_text).expect("Failed to write mode line.");
        self.terminal.ostream.flush().unwrap();
        self.position_cursor(pos.0.into(), pos.1.into());
    }

    pub fn clear_screen(&mut self) {
        write!(self.terminal.ostream, "{}[2J", 27 as char)
            .expect("Write failed in clear_screen().");
        self.terminal
            .ostream
            .flush()
            .expect("Failed post write flush.");
    }

    pub fn keybinding_movement(&mut self, dir: char, lines: Option<u8>) {
        if lines.is_some() {
            write!(self.terminal.ostream, "\x1b[{}{}", lines.unwrap(), dir)
                .expect("Write failed in move_cursor().");
            self.terminal
                .ostream
                .flush()
                .expect("Failed post write flush.")
        } else {
            write!(self.terminal.ostream, "\x1b[1{}", dir).expect("Write failed in move_cursor().");
            self.terminal
                .ostream
                .flush()
                .expect("Failed post write flush.");
        }
        self.terminal.ostream.flush().unwrap();
        self.terminal.istream.lock().consume(1);
    }

    pub fn render_edges(&mut self) {
        write!(self.terminal.ostream, "{}", self.left_edge).unwrap();
        write!(self.terminal.ostream, "{}", self.bottom_line).unwrap();
        write!(self.terminal.ostream, "{}", self.mode_line).unwrap();
        self.terminal.ostream.flush().unwrap();
    }
}

#[derive(Debug)]
pub struct Terminal {
    istream: std::io::Stdin,
    ostream: std::io::Stdout,
    buffer: [u8; 1],
}

impl Terminal {
    pub fn new() -> Self {
        Terminal {
            istream: std::io::stdin(),
            ostream: std::io::stdout(),
            buffer: [0; 1],
        }
    }

    pub fn write_input(&mut self, start: usize, end: usize) {
        self.ostream
            .write_all(&self.buffer[start..end])
            .expect("Failed to write buffer to ostream.");
        self.ostream.flush().expect("Failed post write flush.");
        self.istream.lock().consume(end);
    }

    pub fn read_buffer(&mut self) -> std::io::Result<()> {
        match self.istream.lock().fill_buf() {
            Ok(buf) => {
                self.buffer.copy_from_slice(&buf);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn cr_nl(&mut self, end: usize) {
        self.ostream
            .write_all("\r\n~ ".as_bytes())
            .expect("Failed to write buffer to ostream.");
        self.ostream
            .flush()
            .expect("Failed to unwrap post write flush.");
        self.istream.lock().consume(end);
    }

    pub fn validate_utf8(&mut self, mut start: usize, mut end: usize) {
        while let Err(err) = std::str::from_utf8(&self.buffer[start..end]) {
            if let Some(invalid) = err.error_len() {
                let valid = start + err.valid_up_to();
                write!(self.ostream, "{}", char::REPLACEMENT_CHARACTER)
                    .expect("Failed to write buffer to ostream.");
                start = valid + invalid;
                self.ostream.flush().expect("Failed post write flush.");
                self.istream.lock().consume(end);
            } else {
                // This could be a valid char whose UTF-8 byte sequence is spanning multiple chunks.
                end = start + err.valid_up_to();
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Point {
    row: u16,
    col: u16,
    r: Vec<u8>,
    c: Vec<u8>,
    escape_buf: Vec<u8>, 
}

impl Point {
    fn new() -> Self {
        Point {
            row: 0,
            col: 0,
            r: Vec::with_capacity(5),
            c: Vec::with_capacity(5),
            escape_buf: Vec::with_capacity(14),
        }
    }
}
