#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use core::time::Duration;
use glam::Mat4;
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;

mod camera;
mod render;
mod world;

use camera::Camera;

#[entry]
fn main() -> Status {
    uefi::helpers::init().expect("Failed to init UEFI");

    let gop_handle = get_handle_for_protocol::<GraphicsOutput>().expect("Failed to get GOP handle");

    let mut gop = open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Failed to open GOP");

    let (width, height) = gop.current_mode_info().resolution();
    let mut z_buffer = vec![f32::INFINITY; width * height];
    let mut camera = Camera::new();

    let aspect_ratio = width as f32 / height as f32;
    let projection_matrix = Mat4::perspective_rh_gl(core::f32::consts::FRAC_PI_4, aspect_ratio, 0.1, 100.0);



    // system::with_stdin(|input| {
    //     input.reset(false);
    //     loop {
    //         // Handle input
    //         if let Ok(Some(key)) = input.read_key() {
    //             if let uefi::proto::console::text::Key::Printable(c) = key {
    //                 if u16::from(c) as u8 as char == 'q' {
    //                     break;
    //                 }
    //             }
    //             camera.handle_input(key);
    //         }
    //
    //         // Calculate matrices
    //         let view_matrix = camera.view_matrix();
    //         let view_projection_matrix = projection_matrix * view_matrix;
    //
    //         // Render the scene
    //         render::draw_world(&mut gop, &mut z_buffer, &view_projection_matrix);
    //     }
    // });

    let view_matrix = camera.view_matrix();
    let view_projection_matrix = projection_matrix * view_matrix;
    render::draw_world(&mut gop, &mut z_buffer, &view_projection_matrix);

    boot::stall(Duration::from_mins(2));

    Status::SUCCESS
}
