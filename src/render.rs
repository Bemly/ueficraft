use crate::graphics::Screen;
use core::f32::consts::PI;
use uefi::proto::console::gop::{BltPixel, BltOp, BltRegion, GraphicsOutput};
use uefi::boot::ScopedProtocol;
use libm::{cosf, sinf, tanf, floorf};
use alloc::vec;
use alloc::vec::Vec;
use glam::Vec3;

const WORLD_SIZE_X: usize = 20;
const WORLD_SIZE_Y: usize = 20;
const WORLD_SIZE_Z: usize = 20;

pub type World = [[[u8; WORLD_SIZE_Z]; WORLD_SIZE_Y]; WORLD_SIZE_X];

pub fn create_world() -> World {
    let mut world = [[[0; WORLD_SIZE_Z]; WORLD_SIZE_Y]; WORLD_SIZE_X];
    for x in 0..WORLD_SIZE_X {
        for z in 0..WORLD_SIZE_Z {
            world[x][0][z] = 1; // Floor
        }
    }
    // Place single 1x1x1 blocks
    world[5][1][5] = 2;
    world[10][1][8] = 2;
    world[8][1][3] = 2;
    world[15][1][15] = 2;

    // Add a few more new blocks
    world[7][1][7] = 2;
    world[13][1][6] = 2;
    world[12][1][12] = 2;
    world[6][1][14] = 2;

    world
}

pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

pub fn render(scr: &mut Screen, world: &World, camera: &Camera) {
    let gop = scr.get_gop();
    let (render_width, render_height) = (320, 240);

    let mut frame_buffer = vec![BltPixel::new(0, 0, 0); render_width * render_height];

    let fov = PI / 3.0; // Changed from PI / 2.0 to reduce distortion
    let aspect_ratio = render_width as f32 / render_height as f32;

    for y in 0..render_height {
        for x in 0..render_width {
            let screen_x = (2.0 * (x as f32 / render_width as f32) - 1.0) * aspect_ratio * tanf(fov / 2.0);
            let screen_y = (1.0 - 2.0 * (y as f32 / render_height as f32)) * tanf(fov / 2.0);

            let ray_dir = Vec3::new(screen_x, screen_y, -1.0);
            // Handle yaw and pitch rotation
            let ray_yaw = Vec3::new(
                ray_dir.x * cosf(camera.yaw) - ray_dir.z * sinf(camera.yaw),
                ray_dir.y,
                ray_dir.x * sinf(camera.yaw) + ray_dir.z * cosf(camera.yaw),
            );
            let ray_dir_rot = Vec3::new(
                ray_yaw.x,
                ray_yaw.y * cosf(camera.pitch) - ray_yaw.z * sinf(camera.pitch),
                ray_yaw.y * sinf(camera.pitch) + ray_yaw.z * cosf(camera.pitch),
            );

            let color = cast_ray(camera.pos, ray_dir_rot, world);
            frame_buffer[y * render_width + x] = color;
        }
    }

    let _ = gop.blt(BltOp::BufferToVideo {
        buffer: &frame_buffer,
        src: BltRegion::Full,
        dest: (0, 0),
        dims: (render_width, render_height),
    });
}


fn cast_ray(origin: Vec3, dir: Vec3, world: &World) -> BltPixel {
    let mut map_pos = Vec3::new(floorf(origin.x), floorf(origin.y), floorf(origin.z));

    let delta_dist = Vec3::new(
        if dir.x == 0.0 { 1e30 } else { (1.0 / dir.x).abs() },
        if dir.y == 0.0 { 1e30 } else { (1.0 / dir.y).abs() },
        if dir.z == 0.0 { 1e30 } else { (1.0 / dir.z).abs() },
    );

    let mut step = Vec3::new(0.0, 0.0, 0.0);
    let mut side_dist = Vec3::new(0.0, 0.0, 0.0);

    if dir.x < 0.0 {
        step.x = -1.0;
        side_dist.x = (origin.x - map_pos.x) * delta_dist.x;
    } else {
        step.x = 1.0;
        side_dist.x = (map_pos.x + 1.0 - origin.x) * delta_dist.x;
    }

    if dir.y < 0.0 {
        step.y = -1.0;
        side_dist.y = (origin.y - map_pos.y) * delta_dist.y;
    } else {
        step.y = 1.0;
        side_dist.y = (map_pos.y + 1.0 - origin.y) * delta_dist.y;
    }

    if dir.z < 0.0 {
        step.z = -1.0;
        side_dist.z = (origin.z - map_pos.z) * delta_dist.z;
    } else {
        step.z = 1.0;
        side_dist.z = (map_pos.z + 1.0 - origin.z) * delta_dist.z;
    }

    for _ in 0..8 { // Max distance
        let mut side = 0;
        if side_dist.x < side_dist.y && side_dist.x < side_dist.z {
            side_dist.x += delta_dist.x;
            map_pos.x += step.x;
            side = 0;
        } else if side_dist.y < side_dist.z {
            side_dist.y += delta_dist.y;
            map_pos.y += step.y;
            side = 1;
        } else {
            side_dist.z += delta_dist.z;
            map_pos.z += step.z;
            side = 2;
        }

        let map_x = map_pos.x as isize;
        let map_y = map_pos.y as isize;
        let map_z = map_pos.z as isize;

        if map_x >= 0 && map_x < WORLD_SIZE_X as isize &&
            map_y >= 0 && map_y < WORLD_SIZE_Y as isize &&
            map_z >= 0 && map_z < WORLD_SIZE_Z as isize {
            let block = world[map_x as usize][map_y as usize][map_z as usize];
            if block > 0 {
                 return match block {
                    // New shading logic for 3D effect
                    1 => match side {
                        1 => BltPixel::new(120, 120, 120), // Top face (brightest)
                        0 => BltPixel::new(100, 100, 100), // X-face (medium)
                        _ => BltPixel::new(80, 80, 80),    // Z-face (darkest)
                    },
                    2 => match side {
                        1 => BltPixel::new(0, 220, 0), // Top face (brightest)
                        0 => BltPixel::new(0, 200, 0), // X-face (medium)
                        _ => BltPixel::new(0, 180, 0), // Z-face (darkest)
                    },
                    _ => BltPixel::new(255, 0, 255), // Error color
                };
            }
        }
    }

    BltPixel::new(135, 206, 235) // Sky color
}