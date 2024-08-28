extern crate libc;

use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;

#[repr(C)]
pub struct OpaqueType {
    buffer: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

impl OpaqueType {
    fn constructor() -> *mut libc::c_void {
        std::ptr::null_mut()
    }
}

pub fn tcgetattr(fd: &std::os::fd::RawFd) -> std::io::Result<libc::termios> {
    let mut result = MaybeUninit::<libc::termios>::uninit();
    let status = unsafe { libc::tcgetattr(*fd, result.as_mut_ptr()) };
    if status != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { result.assume_init() })
    }
}
pub fn tcsetattr(fd: &std::os::fd::RawFd) -> std::io::Result<()> {
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
        std::ptr::addr_of_mut!((*ptr).c_cc[libc::VMIN]).write(0);
        std::ptr::addr_of_mut!((*ptr).c_cc[libc::VTIME]).write(1);
    };
    Ok(())
}
pub fn read(fd: std::os::fd::RawFd, opaque_type: *mut libc::c_void) -> std::io::Result<()> {
    let status = unsafe { libc::read(fd, opaque_type, 8) };

    if status >= 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn main() {
    let fd = std::io::stdout().as_raw_fd();
    let old_fd = match tcgetattr(&fd) {
        Ok(fd) => fd,
        Err(err) => {
            println!("ERROR: {}", err);
        }
    };
    loop {
        match read(fd, OpaqueType::constructor()) {
            Ok(()) => (),
            Err(err) => {
                println!("{}", err);
            }
        }
    }
}
