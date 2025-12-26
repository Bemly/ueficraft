#![no_main]
#![no_std]

mod error;
mod graphics;
mod ascii_font;

extern crate alloc;

use core::time::Duration;
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use crate::error::{kernel_panic, ErrorType, Result, OK};
use crate::graphics::Screen;

#[entry]
fn main() -> Status {
    uefi::helpers::init().expect("Failed to init UEFI");

    let mut scr = Screen::new().expect("Failed to init screen");
    if let Err(e) = init(&mut scr) { kernel_panic(scr, e) }

    boot::stall(Duration::from_mins(2));
    Status::SUCCESS
}


fn init(scr: &mut Screen) -> Result {
    throw!(ErrorType::_Reserve);

    OK
}