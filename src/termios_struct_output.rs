
pub fn _tcsetattr(fd: &std::os::fd::RawFd) -> std::io::Result<libc::termios> {
    let mut termios: MaybeUninit<libc::termios> = MaybeUninit::uninit();
    let ptr = termios.as_mut_ptr();
    unsafe {
        std::ptr::addr_of_mut!((*ptr).c_oflag).write(!libc::OPOST);
        std::ptr::addr_of_mut!((*ptr).c_cflag).write(libc::CS8);
        std::ptr::addr_of_mut!((*ptr).c_iflag)
            .write(!(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON));
        std::ptr::addr_of_mut!((*ptr).c_lflag).write(
            !(libc::ICANON)
                | libc::ECHO
                | libc::ECHOE
                | libc::ECHOK
                | libc::ECHONL
                | libc::ISIG
                | libc::IEXTEN,
        );
        std::ptr::addr_of_mut!((*ptr).c_line).write(0);
        std::ptr::addr_of_mut!((*ptr).c_ispeed).write(15);
        std::ptr::addr_of_mut!((*ptr).c_ospeed).write(15);
        std::ptr::addr_of_mut!((*ptr).c_cc[libc::VMIN]).write(0);
        std::ptr::addr_of_mut!((*ptr).c_cc[libc::VTIME]).write(25);
    };
    let status = unsafe { libc::tcsetattr(*fd, libc::TCSADRAIN, termios.as_ptr()) };
    if status != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { termios.assume_init() })
    }
}


// original_termios_output {
//     c_iflag: 17664,
//     c_oflag: 5,
//     c_cflag: 191,
//     c_lflag: 35387,
//     c_line: 0,
//     c_cc: [
//         3,
//         28,
//         127,
//         21,
//         4,
//         0,
//         1,
//         0,
//         17,
//         19,
//         26,
//         0,
//         18,
//         15,
//         23,
//         22,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//     ],
//     c_ispeed: 15,
//     c_ospeed: 15,
// }
// post_setattr_termios_output {
//     c_iflag: 4294965965,
//     c_oflag: 4294967294,
//     c_cflag: 48,
//     c_lflag: 4294967293,
//     c_line: 0,
//     c_cc: [
//         0,
//         0,
//         0,
//         56,
//         0,
//         1,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//         0,
//    ],
//    c_ispeed: 15,
//    c_ospeed: 15,
// }
