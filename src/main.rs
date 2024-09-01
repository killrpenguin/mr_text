#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

extern crate libc;

use mr_text::ffi::*;
use std::io::{BufRead, ErrorKind, Read, Write};

fn main() {
    let mut stream = std::io::stdin();
    let mut istream = stream.lock();
    let mut ostream = std::io::stdout().lock();

    let original_term = match ffi::tc_getattr(istream.by_ref()) {
        Ok(backup) => backup,
        Err(err) => panic!("Error: {}", err),
    };
    let _revert_on_drop = ffi::RevertOnDrop::new(stream.by_ref(), original_term);

    clear_screen(&mut ostream);
    let mut point = Point::default();

    // Failure to configure a raw terminal should stop the program.
    ffi::configure_raw(&mut istream).unwrap();

    match point.top_left(&mut ostream) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::Interrupted => eprintln!("{}", e),
        error => error.unwrap(),
    }
    loop {
        let buffer = match istream.fill_buf() {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            error => error.unwrap(),
        };
        if buffer.is_empty() {
            break;
        }

        // position short circuts on first true.
        let pos = buffer.iter().position(|&buf| buf == 17); // Check for ctrl + q 0x11
        let ret = buffer.iter().position(|&buf| buf == 0x0A);
        let mut start = 0;
        let mut end = pos.unwrap_or(buffer.len());
        while let Err(err) = std::str::from_utf8(&buffer[start..end]) {
            if let Some(invalid) = err.error_len() {
                let valid = start + err.valid_up_to();
                write!(ostream, "{}", char::REPLACEMENT_CHARACTER).unwrap();
                start = valid + invalid;
            } else {
                // This could be a valid char whose UTF-8 byte sequence is spanning multiple chunks.
                end = start + err.valid_up_to();
                break;
            }
        }

        // let rope_node: [u8; 64] = buffer.iter().map(|chr|
        //     match &[chr] {
                
        //     }
        // ).collect();

        if ret.is_some() {
            ostream.write_all("\r\n".as_bytes()).unwrap();
            ostream.flush().expect("Failed to unwrap post write flush.");
            istream.consume(end);
        } else {
            ostream
                .write_all(&buffer[start..end])
                .expect("Failed to write buffer to ostream.");
            ostream.flush().expect("Failed to unwrap post write flush.");
            istream.consume(end);
        }
        if pos.is_some() {
            // exit on ctrl + q
            istream.consume(1);
            break;
        }
    }
    clear_screen(&mut ostream);
}
#[derive(Debug, Default, Copy, Clone)]
pub struct Point {
    row: usize,
    column: usize,
}

impl Point {
    fn new() -> Self {
        Point { row: 1, column: 1 }
    }
    fn top_left(&mut self, ostream: &mut dyn std::io::Write) -> std::io::Result<()> {
        match ostream.write_all(b"\x1b[H") {
            Ok(()) => {
                self.row = 1;
                self.column = 1;
                ostream.flush().unwrap();
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

pub fn clear_screen(ostream: &mut dyn std::io::Write) {
    write!(ostream, "{}[2J", 27 as char).unwrap();
    ostream.flush().unwrap();
}

pub fn draw_rows(ostream: &mut dyn std::io::Write) {
    todo!()
}
