use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::slice;
use glam::{IVec3, Mat4, Vec3, Vec3A, Vec4, Vec4Swizzles};
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive, ScopedProtocol};
use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput, PixelFormat};
use crate::ascii_font::FONT_8X16;
use crate::error::{Result, OK};
use crate::t;
use crate::world::Face;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub struct Screen {
    pub gop: ScopedProtocol<GraphicsOutput>,
    row_ptr: usize,
    pub width: usize,
    pub height: usize,
    pub phys_width: usize,
    pub phys_height: usize,

    // Aligned storage (Vec<u128> ensures 16-byte alignment)
    _zb_store: Vec<u128>,
    pub z_buffer: *mut f32,

    _fb_store: Vec<u128>,
    pub frame_buffer: *mut u32,

    pub half_res: bool,
}

unsafe impl Send for Screen {}
unsafe impl Sync for Screen {}

impl Screen {
    pub fn new() -> Result<Self> {
        let gop = t!(get_handle_for_protocol::<GraphicsOutput>());
        let gop = t!(open_protocol_exclusive::<GraphicsOutput>(gop));
        let (phys_width, phys_height) = gop.current_mode_info().resolution();

        let half_res = true;
        let width = if half_res { phys_width / 2 } else { phys_width };
        let height = if half_res { phys_height / 2 } else { phys_height };

        let size = width * height;
        // Allocate aligned memory
        let zb_len = (size * 4 + 15) / 16;
        let mut zb_store = vec![0u128; zb_len];
        let z_buffer = zb_store.as_mut_ptr() as *mut f32;

        let fb_len = (size * 4 + 15) / 16;
        let mut fb_store = vec![0u128; fb_len];
        let frame_buffer = fb_store.as_mut_ptr() as *mut u32;

        // Init Z-buffer
        unsafe {
            for i in 0..size {
                *z_buffer.add(i) = f32::INFINITY;
            }
            for i in 0..size {
                *frame_buffer.add(i) = 0;
            }
        }

        Ok(Self {
            gop,
            row_ptr: 0,
            width,
            height,
            phys_width,
            phys_height,
            _zb_store: zb_store,
            z_buffer,
            _fb_store: fb_store,
            frame_buffer,
            half_res
        })
    }

    pub fn clear_tile(&self, tile: (usize, usize, usize, usize)) {
        let (min_x, min_y, max_x, max_y) = tile;
        unsafe {
            for y in min_y..max_y {
                let offset = y * self.width + min_x;
                let len = max_x - min_x;
                let z_ptr = self.z_buffer.add(offset);
                let fb_ptr = self.frame_buffer.add(offset);

                for i in 0..len {
                    *z_ptr.add(i) = f32::INFINITY;
                    *fb_ptr.add(i) = 0;
                }
            }
        }
    }

    pub fn flush(&mut self) -> Result {
        let mode = self.gop.current_mode_info();
        let stride = mode.stride();
        let (pw, ph) = mode.resolution();

        let mut fb = self.gop.frame_buffer();
        let base = fb.as_mut_ptr();

        unsafe {
            if self.half_res {
                // Upscale 2x
                for y in 0..self.height {
                    for x in 0..self.width {
                        let color = *self.frame_buffer.add(y * self.width + x);

                        let py = y * 2;
                        let px = x * 2;

                        if py < ph && px < pw {
                            let idx1 = py * stride + px;
                            let ptr1 = base.add(idx1 * 4) as *mut u32;
                            *ptr1 = color;

                            if px + 1 < pw {
                                *ptr1.add(1) = color;
                            }
                        }

                        if py + 1 < ph && px < pw {
                            let idx2 = (py + 1) * stride + px;
                            let ptr2 = base.add(idx2 * 4) as *mut u32;
                            *ptr2 = color;

                            if px + 1 < pw {
                                *ptr2.add(1) = color;
                            }
                        }
                    }
                }
            } else {
                for y in 0..self.height {
                    let src_ptr = self.frame_buffer.add(y * self.width);
                    let dst_ptr = base.add(y * stride * 4) as *mut u32;
                    core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, self.width);
                }
            }
        }

        OK
    }

    pub fn draw_tile(&self, faces: &[Face], view_proj: Mat4, tile: (usize, usize, usize, usize)) {
        let (min_x, min_y, max_x, max_y) = tile;
        let half_width = self.width as f32 * 0.5;
        let half_height = self.height as f32 * 0.5;

        for face in faces {
            let (v0, v1, v2, v3) = get_face_vertices(face);

            let p0 = view_proj * v0.extend(1.0);
            let p1 = view_proj * v1.extend(1.0);
            let p2 = view_proj * v2.extend(1.0);
            let p3 = view_proj * v3.extend(1.0);

            if p0.w <= 0.0 || p1.w <= 0.0 || p2.w <= 0.0 || p3.w <= 0.0 { continue; }

            let ndc0 = p0.xyz() / p0.w;
            let ndc1 = p1.xyz() / p1.w;
            let ndc2 = p2.xyz() / p2.w;
            let ndc3 = p3.xyz() / p3.w;

            // Frustum culling
            if (ndc0.x < -1.0 && ndc1.x < -1.0 && ndc2.x < -1.0 && ndc3.x < -1.0) ||
               (ndc0.x > 1.0 && ndc1.x > 1.0 && ndc2.x > 1.0 && ndc3.x > 1.0) ||
               (ndc0.y < -1.0 && ndc1.y < -1.0 && ndc2.y < -1.0 && ndc3.y < -1.0) ||
               (ndc0.y > 1.0 && ndc1.y > 1.0 && ndc2.y > 1.0 && ndc3.y > 1.0) {
                continue;
            }

            let s0 = Vec3A::new(ndc0.x * half_width + half_width, -ndc0.y * half_height + half_height, ndc0.z);
            let s1 = Vec3A::new(ndc1.x * half_width + half_width, -ndc1.y * half_height + half_height, ndc1.z);
            let s2 = Vec3A::new(ndc2.x * half_width + half_width, -ndc2.y * half_height + half_height, ndc2.z);
            let s3 = Vec3A::new(ndc3.x * half_width + half_width, -ndc3.y * half_height + half_height, ndc3.z);

            // Backface culling (CW check for screen space with Y down)
            if (s1.x - s0.x) * (s2.y - s0.y) - (s1.y - s0.y) * (s2.x - s0.x) <= 0.0 {
                continue;
            }

            let min_fx = min(min(s0.x as i32, s1.x as i32), min(s2.x as i32, s3.x as i32));
            let max_fx = max(max(s0.x as i32, s1.x as i32), max(s2.x as i32, s3.x as i32));
            let min_fy = min(min(s0.y as i32, s1.y as i32), min(s2.y as i32, s3.y as i32));
            let max_fy = max(max(s0.y as i32, s1.y as i32), max(s2.y as i32, s3.y as i32));

            if max_fx < min_x as i32 || min_fx >= max_x as i32 || max_fy < min_y as i32 || min_fy >= max_y as i32 {
                continue;
            }

            let color = get_block_color(face.block);

            #[cfg(target_arch = "x86_64")]
            {
                self.draw_triangle_tile_sse(s0, s1, s2, color, tile);
                self.draw_triangle_tile_sse(s0, s2, s3, color, tile);
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                self.draw_triangle_tile_fixed(s0, s1, s2, color, tile);
                self.draw_triangle_tile_fixed(s0, s2, s3, color, tile);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn draw_triangle_tile_sse(&self, v0: Vec3A, v1: Vec3A, v2: Vec3A, color: u32, tile: (usize, usize, usize, usize)) {
        let (min_tx, min_ty, max_tx, max_ty) = tile;

        let min_x = max(min_tx as i32, min(min(v0.x as i32, v1.x as i32), v2.x as i32));
        let max_x = min(max_tx as i32 - 1, max(max(v0.x as i32, v1.x as i32), v2.x as i32));
        let min_y = max(min_ty as i32, min(min(v0.y as i32, v1.y as i32), v2.y as i32));
        let max_y = min(max_ty as i32 - 1, max(max(v0.y as i32, v1.y as i32), v2.y as i32));

        if min_x > max_x || min_y > max_y { return; }

        unsafe {
            // Edge function: (B-A) x (P-A)
            let edge = |ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32| -> f32 {
                (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
            };

            let area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
            if area <= 0.0 { return; }
            let inv_area = 1.0 / area;

            let v0_z = _mm_set1_ps(v0.z);
            let v1_z = _mm_set1_ps(v1.z);
            let v2_z = _mm_set1_ps(v2.z);
            let inv_area_vec = _mm_set1_ps(inv_area);
            let zero = _mm_setzero_ps();

            // Setup edge constants for (B-A) x (P-A)
            // w = (bx-ax)(py-ay) - (by-ay)(px-ax)
            //   = (bx-ax)py - (bx-ax)ay - (by-ay)px + (by-ay)ax
            //   = px(ay-by) + py(bx-ax) + (ax(by-ay) - ay(bx-ax))
            let setup_edge = |ax: f32, ay: f32, bx: f32, by: f32| -> (__m128, __m128, __m128) {
                let cx = ay - by;
                let cy = bx - ax;
                let cc = ax * (by - ay) - ay * (bx - ax);
                (_mm_set1_ps(cx), _mm_set1_ps(cy), _mm_set1_ps(cc))
            };

            let (w0_cx, w0_cy, w0_cc) = setup_edge(v1.x, v1.y, v2.x, v2.y);
            let (w1_cx, w1_cy, w1_cc) = setup_edge(v2.x, v2.y, v0.x, v0.y);
            let (w2_cx, w2_cy, w2_cc) = setup_edge(v0.x, v0.y, v1.x, v1.y);

            let step_x = _mm_set_ps(3.0, 2.0, 1.0, 0.0);

            for y in min_y..=max_y {
                let py = _mm_set1_ps(y as f32 + 0.5);

                let px_start = _mm_add_ps(_mm_set1_ps(min_x as f32 + 0.5), step_x);

                let mut w0 = _mm_add_ps(_mm_add_ps(_mm_mul_ps(px_start, w0_cx), _mm_mul_ps(py, w0_cy)), w0_cc);
                let mut w1 = _mm_add_ps(_mm_add_ps(_mm_mul_ps(px_start, w1_cx), _mm_mul_ps(py, w1_cy)), w1_cc);
                let mut w2 = _mm_add_ps(_mm_add_ps(_mm_mul_ps(px_start, w2_cx), _mm_mul_ps(py, w2_cy)), w2_cc);

                let dw0 = _mm_mul_ps(_mm_set1_ps(4.0), w0_cx);
                let dw1 = _mm_mul_ps(_mm_set1_ps(4.0), w1_cx);
                let dw2 = _mm_mul_ps(_mm_set1_ps(4.0), w2_cx);

                let mut x = min_x;
                while x <= max_x {
                    let m0 = _mm_cmpge_ps(w0, zero);
                    let m1 = _mm_cmpge_ps(w1, zero);
                    let m2 = _mm_cmpge_ps(w2, zero);
                    let mask = _mm_and_ps(m0, _mm_and_ps(m1, m2));

                    let mask_bits = _mm_movemask_ps(mask);

                    if mask_bits != 0 {
                        let fw0 = _mm_mul_ps(w0, inv_area_vec);
                        let fw1 = _mm_mul_ps(w1, inv_area_vec);
                        let fw2 = _mm_mul_ps(w2, inv_area_vec);

                        let z = _mm_add_ps(_mm_add_ps(_mm_mul_ps(fw0, v0_z), _mm_mul_ps(fw1, v1_z)), _mm_mul_ps(fw2, v2_z));

                        let mut z_vals = [0.0f32; 4];
                        _mm_storeu_ps(z_vals.as_mut_ptr(), z);

                        for i in 0..4 {
                            if (mask_bits & (1 << i)) != 0 {
                                let xi = x + i;
                                if xi <= max_x {
                                    let idx = y as usize * self.width + xi as usize;
                                    let z_val = z_vals[i as usize];
                                    if z_val < *self.z_buffer.add(idx) {
                                        *self.z_buffer.add(idx) = z_val;
                                        *self.frame_buffer.add(idx) = color;
                                    }
                                }
                            }
                        }
                    }

                    w0 = _mm_add_ps(w0, dw0);
                    w1 = _mm_add_ps(w1, dw1);
                    w2 = _mm_add_ps(w2, dw2);
                    x += 4;
                }
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn draw_triangle_tile_fixed(&self, v0: Vec3A, v1: Vec3A, v2: Vec3A, color: u32, tile: (usize, usize, usize, usize)) {
        let (min_tx, min_ty, max_tx, max_ty) = tile;

        let min_x = max(min_tx as i32, min(min(v0.x as i32, v1.x as i32), v2.x as i32));
        let max_x = min(max_tx as i32 - 1, max(max(v0.x as i32, v1.x as i32), v2.x as i32));
        let min_y = max(min_ty as i32, min(min(v0.y as i32, v1.y as i32), v2.y as i32));
        let max_y = min(max_ty as i32 - 1, max(max(v0.y as i32, v1.y as i32), v2.y as i32));

        if min_x > max_x || min_y > max_y { return; }

        const FP_SHIFT: i32 = 8;
        let x0 = (v0.x * 256.0) as i32;
        let y0 = (v0.y * 256.0) as i32;
        let x1 = (v1.x * 256.0) as i32;
        let y1 = (v1.y * 256.0) as i32;
        let x2 = (v2.x * 256.0) as i32;
        let y2 = (v2.y * 256.0) as i32;

        // (B-A) x (C-A)
        let edge = |ax: i32, ay: i32, bx: i32, by: i32, cx: i32, cy: i32| -> i64 {
            (bx as i64 - ax as i64) * (cy as i64 - ay as i64) - (by as i64 - ay as i64) * (cx as i64 - ax as i64)
        };

        let area = edge(x0, y0, x1, y1, x2, y2);
        if area <= 0 { return; }

        let inv_area = 1.0 / area as f32;

        let mut p_y = (min_y << FP_SHIFT) + (1 << (FP_SHIFT - 1));
        let p_x_start = (min_x << FP_SHIFT) + (1 << (FP_SHIFT - 1));

        for y in min_y..=max_y {
            let mut p_x = p_x_start;
            let mut x = min_x;

            let mut w0_row = edge(x1, y1, x2, y2, p_x, p_y);
            let mut w1_row = edge(x2, y2, x0, y0, p_x, p_y);
            let mut w2_row = edge(x0, y0, x1, y1, p_x, p_y);

            let dw0_dx = (y1 - y2) as i64 * 256; // -(y2-y1) = y1-y2
            let dw1_dx = (y2 - y0) as i64 * 256;
            let dw2_dx = (y0 - y1) as i64 * 256;
            // Wait, check dw_dx logic.
            // w = (bx-ax)(py-ay) - (by-ay)(px-ax)
            // dw/dpx = -(by-ay) = ay - by.
            // w0: v1->v2. ay=y1, by=y2. dw0/dx = y1 - y2. Correct.

            while x <= max_x {
                if x + 3 <= max_x {
                    self.process_pixel_fixed(x, y, w0_row, w1_row, w2_row, area, inv_area, v0.z, v1.z, v2.z, color);
                    self.process_pixel_fixed(x+1, y, w0_row + dw0_dx, w1_row + dw1_dx, w2_row + dw2_dx, area, inv_area, v0.z, v1.z, v2.z, color);
                    self.process_pixel_fixed(x+2, y, w0_row + dw0_dx*2, w1_row + dw1_dx*2, w2_row + dw2_dx*2, area, inv_area, v0.z, v1.z, v2.z, color);
                    self.process_pixel_fixed(x+3, y, w0_row + dw0_dx*3, w1_row + dw1_dx*3, w2_row + dw2_dx*3, area, inv_area, v0.z, v1.z, v2.z, color);

                    x += 4;
                    w0_row += dw0_dx * 4;
                    w1_row += dw1_dx * 4;
                    w2_row += dw2_dx * 4;
                } else {
                    self.process_pixel_fixed(x, y, w0_row, w1_row, w2_row, area, inv_area, v0.z, v1.z, v2.z, color);
                    x += 1;
                    w0_row += dw0_dx;
                    w1_row += dw1_dx;
                    w2_row += dw2_dx;
                }
            }
            p_y += 256;
        }
    }

    #[inline(always)]
    fn process_pixel_fixed(&self, x: i32, y: i32, w0: i64, w1: i64, w2: i64, area: i64, inv_area: f32, z0: f32, z1: f32, z2: f32, color: u32) {
        if w0 >= 0 && w1 >= 0 && w2 >= 0 {
            let fw0 = w0 as f32 * inv_area;
            let fw1 = w1 as f32 * inv_area;
            let fw2 = w2 as f32 * inv_area;

            let z = fw0 * z0 + fw1 * z1 + fw2 * z2;

            let idx = y as usize * self.width + x as usize;
            unsafe {
                if z < *self.z_buffer.add(idx) {
                    *self.z_buffer.add(idx) = z;
                    *self.frame_buffer.add(idx) = color;
                }
            }
        }
    }

    pub fn println(&mut self, text: &str) -> Result {
        let mut x = 0;
        let (width, height) = self.gop.current_mode_info().resolution();

        let fg = BltPixel::new(255, 255, 255);
        let bg = BltPixel::new(0, 0, 0);

        if self.row_ptr + 20 >= height { self.row_ptr = 0 }

        for c in text.chars() {
            if c == '\n' {
                x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
                continue;
            }

            if x + 8 > width {
                x = 0;
                self.row_ptr += 18;
                if self.row_ptr >= height { self.row_ptr = 0 }
            }

            let index = (c as usize) & 0x7F;
            let glyph = &FONT_8X16[index];

            for row in 0..16 {
                let row_bits = glyph[row];
                for col in 0..8 {
                    let is_fg = (row_bits >> (7 - col)) & 1 == 1;
                    let color = if is_fg { fg } else { bg };

                    t!(self.gop.blt(BltOp::VideoFill {
                        color,
                        dest: (x + col, self.row_ptr + row),
                        dims: (1, 1),
                    }));
                }
            }
            x += 8;
        }
        self.row_ptr += 18;

        OK
    }
}

fn get_face_vertices(face: &Face) -> (Vec3, Vec3, Vec3, Vec3) {
    let p = face.pos.as_vec3();
    let s = face.size.as_vec3();

    match face.axis {
        0 => ( // -X
            p + Vec3::new(0.0, 0.0, 0.0),
            p + Vec3::new(0.0, 0.0, s.z),
            p + Vec3::new(0.0, s.y, s.z),
            p + Vec3::new(0.0, s.y, 0.0),
        ),
        1 => ( // +X
            p + Vec3::new(1.0, 0.0, s.z),
            p + Vec3::new(1.0, 0.0, 0.0),
            p + Vec3::new(1.0, s.y, 0.0),
            p + Vec3::new(1.0, s.y, s.z),
        ),
        2 => ( // -Y
            p + Vec3::new(0.0, 0.0, 0.0),
            p + Vec3::new(s.x, 0.0, 0.0),
            p + Vec3::new(s.x, 0.0, s.z),
            p + Vec3::new(0.0, 0.0, s.z),
        ),
        3 => ( // +Y
            p + Vec3::new(0.0, 1.0, s.z),
            p + Vec3::new(s.x, 1.0, s.z),
            p + Vec3::new(s.x, 1.0, 0.0),
            p + Vec3::new(0.0, 1.0, 0.0),
        ),
        4 => ( // -Z
            p + Vec3::new(s.x, 0.0, 0.0),
            p + Vec3::new(0.0, 0.0, 0.0),
            p + Vec3::new(0.0, s.y, 0.0),
            p + Vec3::new(s.x, s.y, 0.0),
        ),
        5 => ( // +Z
            p + Vec3::new(0.0, 0.0, 1.0),
            p + Vec3::new(s.x, 0.0, 1.0),
            p + Vec3::new(s.x, s.y, 1.0),
            p + Vec3::new(0.0, s.y, 1.0),
        ),
        _ => (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO),
    }
}

fn get_block_color(block: crate::world::Block) -> u32 {
    use crate::world::Block;
    match block {
        Block::Grass => 0xFF00FF00, // Green
        Block::Dirt => 0xFF8B4513, // Brown
        Block::Stone => 0xFF808080, // Gray
        Block::Bedrock => 0xFF000000, // Black
        _ => 0xFFFFFFFF,
    }
}
