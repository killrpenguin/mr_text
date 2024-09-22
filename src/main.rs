#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]
const CLR_SCRN: &str = "\x1b[2J";

extern crate libc;
use mr_text::{
    ffi,
    screen::{self, Builder, DrawScreen, Screen},
};

fn main() {
    let mut screen = Screen::new()
        .mode_line()
        .left_margin()
        .point()
        .text_window()
        .backup_terminal()
        .build();

    let mut drop_stream = std::io::stdin();
    let _revert_on_drop = ffi::RevertOnDrop::new(&mut drop_stream, screen.copy_original_term());

    Screen::raw_mode();
    screen.clear_screen();
    screen.draw_ml_area();
    screen.draw_numbered_lm();
    loop {
        match screen.ascii_strategy() {
            Some(()) => {}
            None => break,
        }
        screen.update_point::<std::io::Stdout>().unwrap_or(());
        screen.draw_ml_area();
        screen.clr_msg_timer();
        screen.draw_numbered_lm();
    }
    screen.clear_screen();
}

