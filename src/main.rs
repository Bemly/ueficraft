#![no_main]
#![no_std]

mod ascii_font;
mod error;
mod game;
mod render;
mod world;

extern crate alloc;

use core::ffi::c_void;
use core::ptr::addr_of_mut;
use core::time::Duration;
use uefi::boot::{
    create_event, get_handle_for_protocol, open_protocol_exclusive, set_watchdog_timer, EventType,
    Tpl,
};
use uefi::prelude::*;
use uefi::proto::pi::mp::MpServices;

use crate::error::{kernel_panic, OK, Result};
use crate::game::{game_task, GameContext};
use crate::render::Screen;

#[entry]
fn main() -> Status {
    uefi::helpers::init().expect("Failed to init UEFI");

    let mut scr = Screen::new().expect("Failed to init screen");
    if let Err(e) = init(&mut scr) {
        kernel_panic(&mut scr, e);
    }

    boot::stall(Duration::from_mins(2));
    Status::SUCCESS
}


fn init(scr: &mut Screen) -> Result {
    // 防止BIOS把爷程序给中断了
    t!(set_watchdog_timer(0, 0, None));

    let mp = t!(get_handle_for_protocol::<MpServices>());
    let mp = t!(open_protocol_exclusive::<MpServices>(mp));
    let num_cores = t!(mp.get_number_of_processors()).enabled;

    let mut ctx = GameContext {
        mp: &mp,
        scr,
        num_cores,
    };
    let arg_ptr = addr_of_mut!(ctx).cast::<c_void>();

    let event = unsafe { t!(create_event(EventType::empty(), Tpl::CALLBACK, None, None)) };

    if num_cores > 1 {
        let _ = mp.startup_all_aps(false, game_task, arg_ptr, Some(event), None);
    }

    game_task(arg_ptr);

    OK
}