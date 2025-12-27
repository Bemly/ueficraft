use alloc::vec;
use alloc::vec::Vec;
use glam::{Mat3A, Vec2, Vec3A};
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive, ScopedProtocol};
use uefi::proto::console::gop::{BltPixel, FrameBuffer, GraphicsOutput, PixelFormat};
use crate::{t, world};
use crate::ascii_font::FONT_8X16;
use crate::error::OK;

pub const FB_W: usize = 320;
pub const FB_H: usize = 200;

const SKY_COLOR_VEC: Vec3A = Vec3A::new(135.0 / 255.0, 206.0 / 255.0, 235.0 / 255.0);
const FOG_DISTANCE: f32 = 32.0;

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub pos: Vec3A,
    pub velocity: Vec3A,
    pub yaw: f32,
    pub pitch: f32,
    pub is_crouching: bool,
}

pub struct Screen {
    pub gop: ScopedProtocol<GraphicsOutput>,
    row_ptr: usize,
}

impl Screen {
    pub fn new() -> crate::error::Result<Self> {
        let gop = t!(get_handle_for_protocol::<GraphicsOutput>());
        let gop = t!(open_protocol_exclusive::<GraphicsOutput>(gop));
        Ok(Self { gop, row_ptr: 0 })
    }

    pub fn clear(&mut self, color: BltPixel) -> crate::error::Result {
        let (w, h) = self.gop.current_mode_info().resolution();
        let stride = self.gop.current_mode_info().stride();
        let fb_ptr = self.gop.frame_buffer().as_mut_ptr();

        let line: Vec<BltPixel> = vec![color; w];
        let line_bytes = unsafe { core::slice::from_raw_parts(line.as_ptr() as *const u8, w * 4) };

        for y in 0..h {
            unsafe {
                let dest_ptr = fb_ptr.add(y * stride * 4);
                core::ptr::copy_nonoverlapping(line_bytes.as_ptr(), dest_ptr, w * 4);
            }
        }
        OK
    }

    pub fn println(&mut self, text: &str) {
        let (width, height) = self.gop.current_mode_info().resolution();
        let stride = self.gop.current_mode_info().stride();
        let fb_ptr = self.gop.frame_buffer().as_mut_ptr();

        let fg = BltPixel::new(255, 255, 255);
        let bg = BltPixel::new(0, 0, 0);

        if self.row_ptr + 20 >= height { self.row_ptr = 0 }

        let mut current_x = 0;
        for c in text.chars() {
            if c == '\n' {
                current_x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
                continue;
            }

            if current_x + 8 > width {
                current_x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
            }

            let index = (c as usize) & 0x7F;
            let glyph = &FONT_8X16[index];

            for y_offset in 0..16 {
                let row_bits = glyph[y_offset];
                let y = self.row_ptr + y_offset;
                if y >= height { continue; }

                for x_offset in 0..8 {
                    let is_fg = (row_bits >> (7 - x_offset)) & 1 == 1;
                    let x = current_x + x_offset;
                    if x >= width { continue; }

                    let color = if is_fg { fg } else { bg };

                    unsafe {
                        let byte_offset = (y * stride + x) * 4;
                        (fb_ptr.add(byte_offset) as *mut BltPixel).write_volatile(color);
                    }
                }
            }
            current_x += 8;
        }
        self.row_ptr += 18;
    }
}

pub fn ray_march(
    player: &Player,
    start_y: usize,
    end_y: usize,
    fb: &mut FrameBuffer,
    stride: usize,
    (width, height): (usize, usize),
    pixel_format: PixelFormat,
) {
    let rot = Mat3A::from_rotation_y(player.yaw) * Mat3A::from_rotation_x(player.pitch);

    type PixelWriter = unsafe fn(&mut FrameBuffer, usize, [u8; 3]);
    fn write_pixel_rgb(fb: &mut FrameBuffer, pixel_base: usize, rgb: [u8; 3]) {
        unsafe { fb.write_value(pixel_base, rgb) }
    }
    fn write_pixel_bgr(fb: &mut FrameBuffer, pixel_base: usize, rgb: [u8; 3]) {
        unsafe { fb.write_value(pixel_base, [rgb[2], rgb[1], rgb[0]]) } 
    }

    let write_pixel: PixelWriter = match pixel_format {
        PixelFormat::Rgb => write_pixel_rgb,
        PixelFormat::Bgr => write_pixel_bgr,
        _ => return, // 不支持的像素格式
    };

    for y in start_y..end_y {
        for x in 0..width {
            let uv = Vec2::new(x as f32, y as f32) / Vec2::new(width as f32, height as f32) * 2.0 - 1.0;
            let dir = (rot * Vec3A::new(uv.x, -uv.y, 1.0)).normalize();

            let step = dir.signum().as_ivec3();
            let map_pos = player.pos.as_ivec3();
            let delta = (1.0 / dir).abs();

            let mut side_dist = Vec3A::ZERO;
            if dir.x < 0.0 {
                side_dist.x = (player.pos.x - map_pos.x as f32) * delta.x;
            } else {
                side_dist.x = (map_pos.x as f32 + 1.0 - player.pos.x) * delta.x;
            }
            if dir.y < 0.0 {
                side_dist.y = (player.pos.y - map_pos.y as f32) * delta.y;
            } else {
                side_dist.y = (map_pos.y as f32 + 1.0 - player.pos.y) * delta.y;
            }
            if dir.z < 0.0 {
                side_dist.z = (player.pos.z - map_pos.z as f32) * delta.z;
            } else {
                side_dist.z = (map_pos.z as f32 + 1.0 - player.pos.z) * delta.z;
            }

            let mut hit = 0;
            let mut side = 0;
            let mut hit_dist = 0.0;
            let mut current_map_pos = map_pos;

            for _ in 0..128 {
                if side_dist.x < side_dist.y && side_dist.x < side_dist.z {
                    hit_dist = side_dist.x;
                    side_dist.x += delta.x;
                    current_map_pos.x += step.x;
                    side = 0;
                } else if side_dist.y < side_dist.z {
                    hit_dist = side_dist.y;
                    side_dist.y += delta.y;
                    current_map_pos.y += step.y;
                    side = 1;
                } else {
                    hit_dist = side_dist.z;
                    side_dist.z += delta.z;
                    current_map_pos.z += step.z;
                    side = 2;
                }

                if hit_dist > FOG_DISTANCE { break; }

                hit = world::get_block(current_map_pos.x, current_map_pos.y, current_map_pos.z);
                if hit > 0 { break; }
            }

            let final_color_vec = if hit > 0 && hit_dist <= FOG_DISTANCE {
                let mut brightness = 1.0;
                if side == 0 { brightness = 0.8; }
                if side == 2 { brightness = 0.6; }

                let block_color_vec = match hit {
                    1 => Vec3A::new(0.5, 0.5, 0.5), // Stone
                    2 => Vec3A::new(0.2, 0.8, 0.2), // Wall (Green)
                    3 => Vec3A::new(0.6, 0.4, 0.2), // Pillar (Brown)
                    _ => Vec3A::new(1.0, 1.0, 1.0),
                };

                let fog_factor = (hit_dist / FOG_DISTANCE).clamp(0.0, 1.0);
                (block_color_vec * brightness).lerp(SKY_COLOR_VEC, fog_factor)
            } else {
                SKY_COLOR_VEC
            };

            let rgb = [
                (final_color_vec.x * 255.0) as u8,
                (final_color_vec.y * 255.0) as u8,
                (final_color_vec.z * 255.0) as u8,
            ];

            let pixel_index = y * stride + x;
            let pixel_base = 4 * pixel_index;
            unsafe { write_pixel(fb, pixel_base, rgb) };
        }
    }
}
