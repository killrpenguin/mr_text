#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

extern crate libc;
use crate::ffi::*;
use std::io::{BufRead, ErrorKind, Read, Write};

#[derive(Debug)]
pub struct RevertOnDrop<'a> {
    istream: &'a mut std::io::Stdin,
    original_term: libc::termios,
}

impl<'a> RevertOnDrop<'a> {
    pub fn new(istream: &'a mut std::io::Stdin, original_term: libc::termios) -> RevertOnDrop {
        RevertOnDrop {
            istream,
            original_term,
        }
    }
}

impl Drop for RevertOnDrop<'_> {
    fn drop(&mut self) {
        ffi::tc_setattr(self.istream, self.original_term).unwrap();
    }
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
mod ffi {
    use std::io::Error;
    use std::mem::MaybeUninit;
    use std::os::*;

    pub fn tc_getattr(stream: &mut impl fd::AsRawFd) -> std::io::Result<libc::termios> {
        let fd = stream.as_raw_fd();
        let mut result: MaybeUninit<libc::termios> = MaybeUninit::uninit();
        let termios_p = result.as_mut_ptr();
        let tcgetattr_ret = unsafe { libc::tcgetattr(fd, termios_p) };
        if tcgetattr_ret != 0 {
            Err(Error::last_os_error())
        } else {
            Ok(unsafe { result.assume_init() })
        }
    }
    pub fn tc_setattr(
        stream: &mut impl fd::AsRawFd,
        termios: libc::termios,
    ) -> std::io::Result<()> {
        let fd = stream.as_raw_fd();
        let status = unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &termios) };
        if status != 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    pub fn configure_raw(stream: &mut impl fd::AsRawFd) -> std::io::Result<()> {
        let mut termios = tc_getattr(stream).unwrap();
        termios.c_iflag &= !(libc::BRKINT | libc::INPCK | libc::ISTRIP | libc::IXON);
        termios.c_oflag &= !(libc::OPOST | libc::ONLCR);
        termios.c_cflag |= libc::CS8;
        termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);

        match tc_setattr(stream, termios) {
            Ok(()) => Ok(()),
            Err(_) => Err(std::io::Error::last_os_error()),
        }
    }
}
pub fn clear_screen(ostream: &mut dyn std::io::Write) {
    write!(ostream, "{}[2J", 27 as char).unwrap();
    ostream.flush().unwrap();
}

fn main() {
    let mut stream = std::io::stdin();
    let mut istream = stream.lock();
    let mut ostream = std::io::stdout().lock();

    let original_term = match tc_getattr(istream.by_ref()) {
        Ok(backup) => backup,
        Err(err) => panic!("Error: {}", err),
    };
    let _revert_on_drop = RevertOnDrop::new(stream.by_ref(), original_term);

    clear_screen(&mut ostream);
    let mut point = Point::default();

    // Failure to configure a raw terminal should stop the program.
    configure_raw(&mut istream).unwrap();
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
        let pos = buffer.iter().position(|&buf| buf == 0x11); // Check for ctrl + q
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

        ostream
            .write_all(&buffer[start..end])
            .expect("Failed to write buffer to ostream.");
        ostream.flush().expect("Failed to unwrap post write flush.");
        istream.consume(end);

        if pos.is_some() {
            // exit on ctrl + q
            istream.consume(1);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Test is not correctly reverting terminal after asserts.
    // #[test]
    // fn test_config_raw() {
    //     let mut stream = std::io::stdin();
    //     let mut istream = stream.lock();
    //     let original_term = match tc_getattr(istream.by_ref()) {
    //         Ok(backup) => backup,
    //         Err(err) => panic!("Error: {}", err),
    //     };
    //     ffi::configure_raw(&mut istream).unwrap();
    //     let term = tc_getattr(&mut istream).unwrap();
    //     assert_eq!(24836, term.c_iflag);
    //     assert_eq!(4, term.c_oflag);
    //     assert_eq!(191, term.c_cflag);
    //     assert_eq!(2608, term.c_lflag);
    //     assert_eq!(0, term.c_line);
    //     assert_eq!(15, term.c_ispeed);
    //     assert_eq!(15, term.c_ospeed);
    //     let _revert_on_drop = RevertOnDrop::new(stream.by_ref(), original_term);
    // }

    #[test]
    fn test_tcgetattr() {
        let mut stream = std::io::stdin();
        let mut istream = stream.lock();
        let term = ffi::tc_getattr(&mut istream).unwrap();
        assert_eq!(25862, term.c_iflag);
        assert_eq!(5, term.c_oflag);
        assert_eq!(191, term.c_cflag);
        assert_eq!(2619, term.c_lflag);
        assert_eq!(0, term.c_line);
        assert_eq!(15, term.c_ispeed);
        assert_eq!(15, term.c_ospeed);
        let _revert_on_drop = RevertOnDrop::new(stream.by_ref(), term);
    }
}
