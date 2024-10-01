#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

extern crate libc;

use std::io::{Error, ErrorKind, Write};

use crate::{
    document::{Doc, Document},
    event::{Key, ReadKey},
    ffi,
    screen::{Builder, DrawScreen, EscSeq, Screen},
};

pub struct MrText<'a, U: Doc>
where
    U: Doc,
{
    screen: Screen<'a>,
    docs: Vec<U>,
}

impl<'a, U> MrText<'_, U>
where
    U: Doc,
{
    pub fn open_doc(&self, file_name: &str) {
        let mut mr_text = MrText::new();
        if let Ok(doc) = Document::open_doc(file_name) {
            mr_text.docs.push(doc);
        } else {
            mr_text.screen.echo_area_msg("Failed to open file.");
        }
    }

    pub fn new() -> Self {
        let mut screen = Screen::new()
            .mode_line()
            .left_margin()
            .point()
            .text_window()
            .backup_terminal()
            .build();

        Screen::raw_mode();
        screen.clear_screen();
        screen.draw_numbered_lm();
        screen.draw_ml_area();

        MrText {
            screen,
            docs: Vec::with_capacity(20),
        }
    }

    pub fn event_loop(&mut self) {
        loop {
            match std::io::stdin().read_key().next() {
                Some(Ok(Key::CtrlKey('q'))) => break,
                Some(Ok(key @ Key::Letter(..))) => {
                    let mut ostream = std::io::stdout();
                    match write!(ostream, "{}{}", key, EscSeq::GetCursorPos) {
                        Ok(_) => {}
                        Err(err) if err.kind() == ErrorKind::Interrupted => panic!(),
                        Err(_) => println!("{}", Error::last_os_error()),
                    }
                    let _ = ostream.flush();
                }
                Some(Ok(output @ Key::CursorPos(pos))) => {
                    self.screen.draw_cursor_pos(output, pos);
                }
                Some(Ok(Key::AltKey(..))) => continue,
                Some(Ok(Key::CtrlKey(..))) => continue,
                Some(Ok(..)) => continue,
                Some(Err(err)) => println!("{}", err),
                None => break,
            }
        }
    }

    pub fn run(&mut self) {
        self.event_loop();

        self.screen.clear_screen();
        let mut drop_stream = std::io::stdin();
        let _revert_on_drop =
            ffi::RevertOnDrop::new(&mut drop_stream, self.screen.copy_original_term());
    }
}
