#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

use crate::{
    event::{Key, ReadKey},
    ffi,
};
use ropey::Rope;
use std::{
    fmt::Display,
    io::{stdin, stdout, BufRead, Error, ErrorKind, Write},
};

#[derive(Debug)]
pub struct Screen<'a> {
    text_window: TextWindow,
    mode_line: ModeLine<'a>,
    left_margin: LeftMargin<'a>,
    point: Point,
    original_term: Option<libc::termios>,
    winsize_row: u16,
    winsize_col: u16,
}

impl<'a> Screen<'a> {
    pub fn new() -> Screen<'a> {
        let mut istream = std::io::stdin();
        let winsize = match ffi::io_ctl(&mut istream) {
            Ok(winsize) => winsize,
            Err(err) => panic!("Couldn't get screen size. \nError: {}", err),
        };
        Screen {
            text_window: TextWindow::default(),
            mode_line: ModeLine::default(),
            left_margin: LeftMargin::default(),
            point: Point::default(),
            original_term: None,
            winsize_row: winsize.ws_row,
            winsize_col: winsize.ws_col,
        }
    }

    pub fn update_ml_sep(mut self, sep: &'a str) {
        self.mode_line.new_sep(sep);
        self.mode_line.seperator_line.clear();
        for _ in 0..self.winsize_col {
            self.mode_line.seperator_line.push_str(sep);
        }
    }

    pub fn clr_echo_area_timer(&mut self) {
        if self.mode_line.echo_area.msg_timer.elapsed().as_secs()
            == self.mode_line.echo_area.disp_len
        {
            self.mode_line.echo_area.message.clear();
        }
    }

    pub fn echo_area_msg<M>(&mut self, msg: M)
    where
        M: AsRef<str>,
    {
        self.mode_line.echo_area.store_message(msg.as_ref());
    }

    pub fn copy_original_term(&self) -> libc::termios {
        self.original_term.unwrap()
    }

    pub fn echo_area_is_empty(&self) -> bool {
        self.mode_line.echo_area.message.is_empty()
    }

    pub fn echo_area_timer_done(&self) -> bool {
        self.mode_line.echo_area.msg_timer.elapsed().as_secs() == self.mode_line.echo_area.disp_len
    }

    pub fn raw_mode() {
        let mut istream = stdin();
        match ffi::configure_raw(&mut istream) {
            Ok(()) => {}
            Err(err) => panic!("{}", err),
        }
    }

    fn move_cursor<W>(ostream: &mut W, row: u16, col: u16) -> std::io::Result<()>
    where
        W: Write + ?Sized,
    {
        while let Err(e) = write!(ostream, "\x1b[{};{}H", row, col) {
            if e.kind() != ErrorKind::Interrupted {
                return Err(Error::last_os_error());
            }
        }
        while let Err(e) = ostream.flush() {
            if e.kind() != ErrorKind::Interrupted {
                return Err(Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn my_write<W>(ostream: &mut W, text: &str) -> std::io::Result<()>
    where
        W: Write + ?Sized,
    {
        match write!(ostream, "{}", text) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => Screen::my_write(ostream, text)?,
            Err(_) => return Err(Error::last_os_error()),
        }
        while let Err(e) = ostream.flush() {
            if e.kind() != ErrorKind::Interrupted {
                return Err(Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn scroll(&mut self, dir: Scroll) {
        let mut ostream = stdout();
        let dir_settings = match dir {
            Scroll::Up => (SCROLL_UP, 0, 0, ""),
            Scroll::Down => (
                SCROLL_DOWN,
                self.mode_line.sep_line_pos - 1,
                0,
                CLR_SCRN_CURSR_END,
            ),
        };
        match write!(
            ostream,
            "{}{}{}\x1b[{};{}H{}",
            HIDE_CURSOR,
            dir_settings.3,
            dir_settings.0,
            dir_settings.1,
            dir_settings.2,
            self.left_margin.indicator
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.scroll(dir),
            Err(_) => self.mode_line.echo_area.store_error(Error::last_os_error()),
        }
        self.draw_ml_area();
        let _ = self.point.go_home(&mut ostream);
    }
}

pub enum Scroll {
    Up,
    Down,
}

pub trait DrawScreen {
    fn draw_screen(&mut self);
    fn draw_ml_area(&mut self);
    fn draw_ml(&mut self);
    fn draw_numbered_lm(&mut self);
    fn draw_cursor_pos(&mut self, pos: Key, pos: (u16, u16));
    fn clear_screen(&mut self);
}

impl DrawScreen for Screen<'_> {
    fn draw_cursor_pos(&mut self, output: Key, pos: (u16, u16)) {
        self.point.row = pos.0;
        self.point.col = pos.1;
        let mut ostream = stdout();
        match write!(
            ostream,
            "{}{}{}{}",
            EscSeq::HideCursor,
            EscSeq::MoveCursor((self.winsize_row, 0)),
            EscSeq::ClrLn,
            output
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.clear_screen(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        match ostream.flush() {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => {
                ostream.flush().expect("Interrupted then failed to unwrap.")
            }
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        let _ = self.point.go_home(&mut ostream);
    }

    fn clear_screen(&mut self) {
        let mut ostream = stdout();
        match write!(ostream, "{}", CLR_SCRN,) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.clear_screen(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        match ostream.flush() {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => {
                ostream.flush().expect("Interrupted then failed to unwrap.")
            }
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
    }

    fn draw_numbered_lm(&mut self) {
        let mut ostream = stdout();
        for (num, row) in (1..=self.point.row).rev().enumerate() {
            match write!(
                ostream,
                "{}\x1b[{};{}H{}\x1b[{};{}H{}",
                HIDE_CURSOR, row, self.left_margin.thickness, CLR_LN_UPTO_CURSR, row, 1, num
            ) {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::Interrupted => self.clear_screen(),
                Err(err) => self.mode_line.echo_area.store_error(err),
            }
        }
        for (num, row) in (self.point.row..self.winsize_row - 1).enumerate() {
            match write!(
                ostream,
                "{}\x1b[{};{}H{}\x1b[{};{}H{}",
                HIDE_CURSOR, row, self.left_margin.thickness, CLR_LN_UPTO_CURSR, row, 1, num
            ) {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::Interrupted => self.clear_screen(),
                Err(err) => self.mode_line.echo_area.store_error(err),
            }
        }
        match write!(
            ostream,
            "\x1b[{};{}H{}{}",
            self.point.row, 1, CLR_LN_UPTO_CURSR, self.left_margin.indicator
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.clear_screen(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        let _ = self.point.go_home(&mut ostream);
    }

    fn draw_screen(&mut self) {
        let mut ostream = stdout();
        match write!(
            ostream,
            "{}{}{}{}",
            HIDE_CURSOR, CLR_SCRN, self.left_margin.seperator_line, &self.mode_line.start_greeting
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.draw_screen(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        let _ = self.point.go_home(&mut ostream);
    }

    fn draw_ml_area(&mut self) {
        let mut ostream = stdout();
        match write!(
            ostream,
            "{}\x1b[{};{}H{}{}",
            HIDE_CURSOR,
            self.mode_line.sep_line_pos,
            0,
            CLR_SCRN_CURSR_END,
            &self.mode_line.seperator_line,
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.draw_screen(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }
        self.draw_ml();
    }

    fn draw_ml(&mut self) {
        let mut ostream = stdout();
        match write!(
            ostream,
            "{}\x1b[{};{}H{}R: {}, C: {}",
            HIDE_CURSOR, self.winsize_row, 0, CLR_LN, self.point.row, self.point.col
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.draw_ml(),
            Err(err) => self.mode_line.echo_area.store_error(err),
        }

        if !self.mode_line.echo_area.message.is_empty() {
            match write!(
                ostream,
                "\x1b[{};{}H{}",
                self.winsize_row, self.mode_line.msg_pos, self.mode_line.echo_area.message
            ) {
                Ok(_) => {}
                Err(err) if err.kind() == ErrorKind::Interrupted => self.draw_ml(),
                Err(err) => self.mode_line.echo_area.store_error(err),
            }
        }
        let _ = self.point.go_home(&mut ostream);
    }
}

pub trait Builder {
    fn text_window(self) -> Self;
    fn mode_line(self) -> Self;
    fn left_margin(self) -> Self;
    fn backup_terminal(self) -> Self;
    fn point(self) -> Self;
    fn build(self) -> Self;
}

impl Builder for Screen<'_> {
    fn build(self) -> Self {
        Screen {
            text_window: self.text_window,
            mode_line: self.mode_line,
            left_margin: self.left_margin,
            point: self.point,
            original_term: self.original_term,
            arena: self.arena,
            winsize_row: self.winsize_row,
            winsize_col: self.winsize_col,
        }
    }

    fn text_window(mut self) -> Self {
        self.text_window = TextWindow {
            rope: Rope::new(),
            bottom_ln: self.winsize_row - self.mode_line.thickness,
            // TODO: rope slice for displaying current
            cur_line: 0,
        };
        self
    }

    fn mode_line(mut self) -> Self {
        let mut bite_buf = [0; 4];
        let sep_as_str = '='.encode_utf8(&mut bite_buf);
        for _ in 0..self.winsize_col {
            self.mode_line.seperator_line.push_str(sep_as_str);
        }

        self.mode_line = ModeLine {
            seperator: "=",
            thickness: 3,
            seperator_line: self.mode_line.seperator_line,
            sep_line_pos: self.winsize_row - 1,
            msg_pos: self.winsize_col / 6,
            start_greeting: "Hi David!".to_string(),
            echo_area: EchoArea::default(),
        };
        self
    }

    fn left_margin(mut self) -> Self {
        let winsize_row = 60;
        let mut new_line = std::string::String::new();
        let rng: (i32, i32) = ((winsize_row as i32) * -1, winsize_row as i32);
        let mut disp_number = (rng.0..rng.1).into_iter();
        let mut term_row = (1..=rng.1).into_iter();

        loop {
            let mut num_row: (i32, i32) = match (disp_number.next(), term_row.next()) {
                (Some(num), Some(row)) => (num, row),
                (Some(_), None) => {
                    term_row = (1..=rng.1).into_iter();
                    (0, 0)
                }
                _ => break,
            };
            if num_row.0.is_negative() {
                num_row.0 = num_row.0 * -1;
            }
            if num_row.0 != 0 {
                new_line.push_str(&format!(
                    "{}\x1b[{};{}H{}\x1b[{};{}H{}",
                    HIDE_CURSOR,
                    num_row.1,
                    self.left_margin.thickness,
                    CLR_LN_UPTO_CURSR,
                    num_row.1,
                    1,
                    num_row.0
                ));
            } else {
                new_line.push_str(&format!(
                    "{}\x1b[{};{}H{}\x1b[{};{}H {}",
                    HIDE_CURSOR,
                    num_row.1,
                    self.left_margin.thickness,
                    CLR_LN_UPTO_CURSR,
                    num_row.1,
                    1,
                    self.left_margin.indicator
                ));
            }
        }

        self.left_margin = LeftMargin {
            indicator: "=>",
            thickness: 4,
            seperator_line: new_line,
        };
        self
    }

    fn point(mut self) -> Self {
        self.point = Point {
            row: 1,
            col: self.left_margin.thickness,
        };
        self
    }

    fn backup_terminal(mut self) -> Self {
        let mut istream = std::io::stdin();
        self.original_term = match ffi::tc_getattr(&mut istream) {
            Ok(backup) => Some(backup),
            Err(err) => panic!("Error: {}", err),
        };
        self
    }
}

/// Use the screen builder method to construct Point. This private struct is used
/// inside ModeLine and should not be accessed directly.
#[derive(Debug, Default, Clone)]
pub struct Point {
    row: u16,
    col: u16,
}

impl Point {
    pub fn go_home<W>(&mut self, ostream: &mut W) -> std::io::Result<()>
    where
        W: Write + ?Sized,
    {
        match write!(ostream, "\x1b[{};{}H{}", self.row, self.col, SHOW_CURSOR) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.go_home(ostream)?,
            Err(err) => return Err(err),
        }
        match ostream.flush() {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => ostream.flush()?,
            Err(err) => return Err(err),
        }
        Ok(())
    }
}

/// Use the screen builder method to construct a TextWindow. It is important that the builder
/// method be used after ModeLine, LeftMargin and Point in any screen builder sequence where the
#[derive(Debug, Default, Clone)]
pub struct TextWindow {
    bottom_ln: u16,
    rope: ropey::Rope,
    cur_line: usize,
}

/// Use the screen builder method to construct a ModeLine. The initial field value of
/// ModeLine thickness is 3 and the initial seperator is "=". Example shows how to change these settings.
#[derive(Debug, Default, Clone)]
pub struct ModeLine<'a> {
    thickness: u16,
    seperator: &'a str,
    seperator_line: std::string::String,
    sep_line_pos: u16,
    msg_pos: u16,
    start_greeting: std::string::String,
    echo_area: EchoArea,
}

impl<'a> ModeLine<'a> {
    pub fn new_sep(&mut self, sep: &'a str) {
        self.seperator = sep
    }

    pub fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }

    pub fn draw_mode_line<W>(&mut self, ostream: &mut W) -> std::io::Result<()>
    where
        W: Write + ?Sized,
    {
        match write!(
            ostream,
            "\x1b[{};{}H {}",
            self.sep_line_pos, 0, self.seperator_line,
        ) {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => self.draw_mode_line(ostream)?,
            Err(err) => return Err(err),
        };
        match ostream.flush() {
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => ostream.flush()?,
            Err(err) => return Err(err),
        }
        Ok(())
    }
}

/// Use the screen builder method to construct an EchoArea. The disp_len field adjusts the amout of time
/// the message will be displayed in the mode line, measured in seconds.
#[derive(Debug, Clone)]
struct EchoArea {
    errors: Rope,
    message: std::string::String,
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
            errors: Rope::new(),
            message: std::string::String::with_capacity(64),
            msg_timer: std::time::Instant::now(),
            disp_len: 3,
        }
    }
}
impl EchoArea {
    pub fn new_disp_len(&mut self, len: u64) {
        self.disp_len = len
    }

    fn store_error<T>(&mut self, err: T)
    where
        T: Display,
    {
        let error = format!("Error: {}", err);
        self.errors.insert(self.errors.len_chars(), &error);
        // Insert the \n seperately so the error doesn't shift the screen when displayed in echo area.
        self.errors.insert(self.errors.len_chars(), "\n");
        self.message.clear();
        if error.len() > 64 {
            self.message = "You've got an error.".to_string();
        } else {
            self.message = error;
        }
        self.store_message("Mr-Text found an error.");
    }

    fn store_message(&mut self, msg: &str) {
        self.msg_timer = std::time::Instant::now();
        if msg.len() < 64 {
            self.message.push_str(msg);
        } else {
            self.message.push_str(&msg[..64]);
        }
    }
}

/// Use the Screen builder method to construct a new LeftMargin. The thickness field is measured in
/// terminal rows. Each row is the thickness of one terminal character. The initial field value of LeftMargin
/// thickness is 2, meaning two rows or two characters. The initial seperator is "~".
#[derive(Debug)]
pub struct LeftMargin<'a> {
    thickness: u16,
    indicator: &'a str,
    seperator_line: std::string::String,
}

impl<'a> std::default::Default for LeftMargin<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> LeftMargin<'a> {
    pub fn new() -> Self {
        LeftMargin {
            thickness: 0,
            indicator: " =>",
            seperator_line: String::new(),
        }
    }
    pub fn new_sep(&mut self, indicator: &'a str) {
        self.indicator = indicator
    }

    pub fn new_thickness(&mut self, thickness: u16) {
        self.thickness = thickness
    }
}

/// # Safety
/// If part of the buf argument in returns invalid this function will use the unsafe from_utf8_unchecked
/// on the valid portion of the code to continue the conversion. The valid subslice is
/// split before the error index and and has already been validated.
pub fn from_utf8_escape_seq(buf: &[u8], start: usize, end: usize) -> Option<&str> {
    match std::str::from_utf8(&buf[start..end]) {
        Ok(row) => Some(row),
        Err(error) => {
            // TODO: Again. What to do with after_valid?
            let (valid, _after_valid) = buf.split_at(error.valid_up_to());
            if valid.len() > ESC_SEQ_LEN {
                Some(unsafe { std::str::from_utf8_unchecked(valid) })
            } else {
                None
            }
        }
    }
}

pub enum EscSeq {
    ClrScrn,          //  "\x1b[2J";
    ClrLn,            //  "\x1b[2K";
    ClrScrnCursrEnd,  //  "\x1b[0J";
    ClrLnCursrEnd,    //  "\x1b[0K";
    ClrLnBeforeCursr, //  "\x1b[1K";

    GetCursorPos,           //  "\x1b[6n";
    MoveCursor((u16, u16)), // "\x1b[{};{}H"
    ShowCursor,             //  "\x1b[?25h";
    HideCursor,             //  "\x1b[?25l";

    ScrollUp,   //  "\x1b[1T";
    ScrollDown, //  "\x1b[1S";
    MvLeft,     //  "\x1b[1D";
    MvRight,    //  "\x1b[1C";
    MvUp,       //  "\x1b[1A";
    MvDown,     //  "\x1b[1B";
}

impl std::fmt::Display for EscSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            EscSeq::ClrScrn => write!(f, "\x1b[2J"),
            EscSeq::ClrLn => write!(f, "\x1b[2K"),
            EscSeq::ClrScrnCursrEnd => write!(f, "\x1b[0J"),
            EscSeq::ClrLnCursrEnd => write!(f, "\x1b[0K"),
            EscSeq::ClrLnBeforeCursr => write!(f, "\x1b[1K"),
            EscSeq::GetCursorPos => write!(f, "\x1b[6n"),
            EscSeq::MoveCursor((r, c)) => write!(f, "\x1b[{};{}H", r, c),
            EscSeq::ShowCursor => write!(f, "\x1b[?25h"),
            EscSeq::HideCursor => write!(f, "\x1b[?25l"),
            EscSeq::ScrollUp => write!(f, "\x1b[1T"),
            EscSeq::ScrollDown => write!(f, "\x1b[1S"),
            EscSeq::MvLeft => write!(f, "\x1b[1D"),
            EscSeq::MvRight => write!(f, "\x1b[1C"),
            EscSeq::MvUp => write!(f, "\x1b[1A"),
            EscSeq::MvDown => write!(f, "\x1b[1B"),
        }
    }
}

// Readability constants.
const CLR_SCRN_CURSR_END: &str = "\x1b[0J";
const CLR_SCRN: &str = "\x1b[2J";
const CLR_LN_CURSR_END: &str = "\x1b[0K";
const CLR_LN_UPTO_CURSR: &str = "\x1b[1K";
const CLR_LN: &str = "\x1b[2K";

const REQ_CURSOR_POS: &str = "\x1b[6n";
const SCROLL_DOWN: &str = "\x1b[1S";
const SCROLL_UP: &str = "\x1b[1T";
const SHOW_CURSOR: &str = "\x1b[?25h";
const HIDE_CURSOR: &str = "\x1b[?25l";

const MV_LEFT: &str = "\x1b[1D";
const MV_RIGHT: &str = "\x1b[1C";
const MV_UP: &str = "\x1b[1A";
const MV_DOWN: &str = "\x1b[1B";

const ESC_SEQ_LEN: usize = 5;
const SEMICOLON: u8 = 59;
