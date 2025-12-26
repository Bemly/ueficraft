use glam::{Mat4, Vec3};
use libm::{ceilf, floorf};
use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};
use crate::world::*;

// Helper for barycentric coordinates
fn edge_function(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

/// Draws a filled triangle with depth testing.
fn draw_triangle(
    gop: &mut GraphicsOutput,
    z_buffer: &mut [f32],
    v0: Vec3, v1: Vec3, v2: Vec3,
    color: BltPixel,
) {
    let (width, height) = gop.current_mode_info().resolution();

    // Screen-space back-face culling
    let area = edge_function(v0, v1, v2);
    if area < 0.0 { return; }

    // Bounding box
    let min_x = (floorf(v0.x.min(v1.x.min(v2.x))) as isize).clamp(0, width as isize - 1);
    let max_x = (ceilf(v0.x.max(v1.x.max(v2.x))) as isize).clamp(0, width as isize - 1);
    let min_y = (floorf(v0.y.min(v1.y.min(v2.y))) as isize).clamp(0, height as isize - 1);
    let max_y = (ceilf(v0.y.max(v1.y.max(v2.y))) as isize).clamp(0, height as isize - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, 0.0);
            let w0 = edge_function(v1, v2, p);
            let w1 = edge_function(v2, v0, p);
            let w2 = edge_function(v0, v1, p);

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let w0 = w0 / area;
                let w1 = w1 / area;
                let w2 = w2 / area;

                let depth = w0 * v0.z + w1 * v1.z + w2 * v2.z;
                let z_idx = y as usize * width + x as usize;

                if depth < z_buffer[z_idx] {
                    z_buffer[z_idx] = depth;
                    gop.blt(BltOp::VideoFill {
                        color,
                        dest: (x as usize, y as usize),
                        dims: (1, 1),
                    }).ok();
                }
            }
        }
    }
}

pub fn draw_world(
    gop: &mut GraphicsOutput,
    z_buffer: &mut [f32],
    view_projection_matrix: &Mat4,
) {
    let (width, height) = gop.current_mode_info().resolution();
    let bg = BltPixel::new(64, 128, 192);

    // Clear buffers
    gop.blt(BltOp::VideoFill { color: bg, dest: (0, 0), dims: (width, height) }).unwrap();
    z_buffer.fill(f32::INFINITY);

    // Render world
    for x in 0..WORLD_SIZE {
        for y in 0..WORLD_SIZE {
            for z in 0..WORLD_SIZE {
                if WORLD[x][y][z] == 1 {
                    let model_matrix = Mat4::from_translation(Vec3::new(x as f32, y as f32, z as f32));
                    let mvp = *view_projection_matrix * model_matrix;

                    for (face_idx, tri_indices) in CUBE_INDICES.chunks_exact(3).enumerate() {
                        let mut screen_coords = [Vec3::ZERO; 3];
                        let mut is_behind_camera = false;

                        for (i, &vertex_index) in tri_indices.iter().enumerate() {
                            let vertex = CUBE_VERTICES[vertex_index];
                            let clip_coord = mvp * vertex.extend(1.0);

                            if clip_coord.w <= 0.0 {
                                is_behind_camera = true;
                                break;
                            }

                            let ndc = clip_coord.truncate() / clip_coord.w;
                            screen_coords[i] = Vec3::new(
                                (ndc.x + 1.0) * 0.5 * width as f32,
                                (1.0 - ndc.y) * 0.5 * height as f32,
                                ndc.z,
                            );
                        }

                        if !is_behind_camera {
                             // Lighting calculation
                            let world_normal = FACE_NORMALS[face_idx / 2];
                            let intensity = world_normal.dot(LIGHT_DIRECTION).max(0.0) * 0.8 + 0.2; // Ambient + Diffuse
                            let base_color = FACE_COLORS[face_idx / 2];
                            let final_color = BltPixel::new(
                                (base_color.red as f32 * intensity) as u8,
                                (base_color.green as f32 * intensity) as u8,
                                (base_color.blue as f32 * intensity) as u8,
                            );

                            draw_triangle(
                                gop,
                                z_buffer,
                                screen_coords[0], screen_coords[1], screen_coords[2],
                                final_color
                            );
                        }
                    }
                }
            }
        }
    }
}
