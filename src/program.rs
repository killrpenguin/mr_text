#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

extern crate libc;

use std::{
    env,
    io::{BufRead, Error, ErrorKind, Write},
};

use crate::{
    document::{self, Doc, Document, NewDocument},
    event::{self, Key, KeyEvent, ReadKey},
    ffi,
    screen::{self, Builder, DrawScreen, Screen},
};

pub struct MrText<'a, U: Doc>
where
    U: Doc,
{
    screen: Screen<'a>,
    docs: Vec<U>,
    quit: Option<()>,
}

impl<'a, U> MrText<'_, U>
where
    U: Doc,
{
    pub fn open_doc(&self, file_name: &str) {
        let mut mr_text = MrText::new();
        if let Ok(doc) = Document::load_file(file_name) {
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
            quit: Some(()),
        }
    }

    pub fn event_loop(&mut self) {
        loop {
            match std::io::stdin().read_key().next() {
                Some(Ok(Key::CtrlKey('q'))) => break,
                Some(Ok(Key::AltKey(..))) => println!("unimplemented"),
                Some(Ok(Key::CtrlKey(..))) => println!("unimplemented"),
                Some(Err(err)) => println!("{}", err),
                Some(Ok(key)) => {
                    let mut ostream = std::io::stdout();
                    match write!(ostream, "{}", key) {
                        Ok(_) => {}
                        Err(err) if err.kind() == ErrorKind::Interrupted => panic!(),
                        Err(_) => println!("{}", Error::last_os_error()),
                    }
                    let _ = ostream.flush();
                }
                None => break,
            }
        }
    }

    pub fn quit(&mut self) {
        self.quit.take().expect("How could this fail?")
    }

    pub fn run(&mut self) {
        loop {
            match self.screen.ascii_strategy() {
                Some(()) => {}
                None => break,
            }
            self.screen.update_point::<std::io::Stdout>().unwrap_or(());
            self.screen.draw_numbered_lm();
            self.screen.draw_ml_area();
            self.screen.clr_echo_area_timer();
        }

        self.screen.clear_screen();
        let mut drop_stream = std::io::stdin();
        let _revert_on_drop =
            ffi::RevertOnDrop::new(&mut drop_stream, self.screen.copy_original_term());
    }
}

pub enum ProgramState {
    IO,
    KeyChord(Key),
}

// The following states can be the T in Program
#[derive(Debug, Default, Clone)]
pub struct KeyChordState {
    keys: Vec<Key>,
}

impl<'a> KeyChordState {
    fn new() -> Self {
        KeyChordState {
            keys: Vec::with_capacity(10),
        }
    }
}
