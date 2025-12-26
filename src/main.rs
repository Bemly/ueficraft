#![no_main]
#![no_std]

mod error;
mod graphics;
mod ascii_font;
mod render;

extern crate alloc;

use core::time::Duration;
use glam::Vec3;
use uefi::boot::set_watchdog_timer;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use libm::{sinf, cosf};

use crate::error::{kernel_panic, Result, OK};
use crate::graphics::Screen;
use crate::render::Camera;

#[entry]
fn main() -> Status {
    uefi::helpers::init().expect("Failed to init UEFI");

    let mut scr = Screen::new().expect("Failed to init screen");
    if let Err(e) = init(&mut scr) { kernel_panic(scr, e) }

    boot::stall(Duration::from_secs(1));
    Status::SUCCESS
}


fn init(scr: &mut Screen) -> Result {
    t!(set_watchdog_timer(0, 0, None));

    let world = render::create_world();
    let mut camera = Camera {
        pos: Vec3::new(10.0, 1.5, 10.0),
        yaw: 0.0,
        pitch: 0.0
    };

    // Game loop
    loop {
        render::render(scr, &world, &camera);

        system::with_stdin(|input| {
            let mut events = [input.wait_for_key_event().unwrap()];
            t!(boot::wait_for_event(&mut events).discard_errdata());

            // Handle input
            if let Ok(Some(key)) = input.read_key() {
                let move_speed = 0.3;
                let rot_speed = 0.08;

                match key {
                    Key::Printable(wide_char) => match wide_char.into() {
                        'w' => {
                            camera.pos.x -= sinf(camera.yaw) * move_speed;
                            camera.pos.z -= cosf(camera.yaw) * move_speed;
                        }
                        's' => {
                            camera.pos.x += sinf(camera.yaw) * move_speed;
                            camera.pos.z += cosf(camera.yaw) * move_speed;
                        }
                        'a' => {
                            camera.pos.x -= cosf(camera.yaw) * move_speed;
                            camera.pos.z += sinf(camera.yaw) * move_speed;
                        }
                        'd' => {
                            camera.pos.x += cosf(camera.yaw) * move_speed;
                            camera.pos.z -= sinf(camera.yaw) * move_speed;
                        }
                        'q' => camera.yaw += rot_speed,
                        'e' => camera.yaw -= rot_speed,
                        _ => {}
                    },
                    Key::Special(special_key) => match special_key {
                        ScanCode::RIGHT => camera.yaw += rot_speed,
                        ScanCode::LEFT => camera.yaw -= rot_speed,
                        ScanCode::UP => camera.pitch += rot_speed,
                        ScanCode::DOWN => camera.pitch -= rot_speed,
                        ScanCode::PAGE_UP => camera.pos.y += move_speed,
                        ScanCode::PAGE_DOWN => camera.pos.y -= move_speed,
                        _ => {},
                    },
                }
            };

            Ok(())
        })?;
    }

    OK
}