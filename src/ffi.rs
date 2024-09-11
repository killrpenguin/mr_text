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
    /// The docs for libc tcsetattr() say that the function returns 0 if any flag is set.
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

}
