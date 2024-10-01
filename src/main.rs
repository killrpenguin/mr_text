#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

extern crate libc;
use std::env;

use mr_text::{
    program,
    document::{self, Doc, Document, NewDocument},
    ffi,
    screen::{self, Builder, DrawScreen, Screen},
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut mr_text = program::MrText::<NewDocument>::new();

    if args.is_empty() {
        mr_text.run();
    } else {
        mr_text.open_doc(&args[0]);
        mr_text.run();
    }
}
