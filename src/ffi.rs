pub mod ffi {
    use std::io::Error;
    use std::mem::MaybeUninit;
    use std::os::*;
    ///  #Safety
    ///
    ///  This function can only return initialized memory or an error.
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
    ///  #Safety
    ///
    ///  This function can only return initialized memory or an error.
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
    /// #Safety
    ///
    /// This function can only return initialized memory or an error.
    ///
    /// The docs for tcsetattr() say that the function returns 0 if any flag is set.
    /// It does not validate that all the flags have been set successfully.
    pub fn configure_raw(stream: &mut impl fd::AsRawFd) -> std::io::Result<()> {
        let mut termios = tc_getattr(stream).unwrap();
        termios.c_iflag &= !(libc::BRKINT | libc::INPCK | libc::ISTRIP | libc::IXON);
        termios.c_oflag &= !(libc::OPOST);
        termios.c_cflag |= libc::CS8;
        termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);

        match tc_setattr(stream, termios) {
            Ok(()) => Ok(()),
            Err(_) => Err(std::io::Error::last_os_error()),
        }
    }
    /// #Safety
    ///
    /// This function can only return initialized memory or an error.
    ///
    /// When the window size changes, a SIGWINCH signal is sent to the
    /// foreground process group.
    ///
    pub fn io_ctl(stream: &mut impl fd::AsRawFd) -> std::io::Result<libc::winsize> {
        let fd = stream.as_raw_fd();
        let mut result: MaybeUninit<libc::winsize> = MaybeUninit::uninit();
        let winsize_p = result.as_mut_ptr();
        let ioctl_ret = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, winsize_p) };
        if ioctl_ret != 0 {
            Err(Error::last_os_error())
        } else {
            Ok(unsafe { result.assume_init() })
        }
    }
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
            tc_setattr(self.istream, self.original_term).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
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
        let _revert_on_drop = ffi::RevertOnDrop::new(stream.by_ref(), term);
    }
}
