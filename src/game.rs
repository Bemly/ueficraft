use core::ffi::c_void;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use uefi::proto::pi::mp::MpServices;
use uefi::proto::console::gop::{BltPixel, BltRegion};
use alloc::vec::Vec;
use glam::{Vec3, vec3, Mat4};
use crate::error::{kernel_panic, OK, Result};
use crate::render::Screen;
use crate::world::World;
use crate::t;

static PANIC_STATE: AtomicBool = AtomicBool::new(false);
static NEXT_TILE: AtomicUsize = AtomicUsize::new(0);
static DRAW_LOCK: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub struct GameContext<'bemly_> {
    pub mp: &'bemly_ MpServices,
    pub scr: &'bemly_ mut Screen,
    pub num_cores: usize,
    pub world: World,
}

pub extern "efiapi" fn game_task(arg: *mut c_void) {
    if arg.is_null() { return; }
    let ctx = unsafe { &mut *arg.cast::<GameContext>() };
    if let Err(e) = run(ctx) {
        // 保证崩溃状态原子性 Acquire-AcqRelease-Acquire 双向屏障
        if PANIC_STATE.compare_exchange(
            false, true, Ordering::AcqRel, Ordering::Acquire
        ).is_ok() {
            kernel_panic(&mut ctx.scr, e)
        }
    }
}

pub fn run(ctx: &mut GameContext) -> Result {
    let (width, height) = ctx.scr.gop.current_mode_info().resolution();

    // Camera setup
    let camera_pos = vec3(2.0, 3.0, 5.0);
    let camera_target = vec3(0.0, 0.0, 0.0);
    let camera_up = vec3(0.0, 1.0, 0.0);

    let view = Mat4::look_at_rh(camera_pos, camera_target, camera_up);
    let projection = Mat4::perspective_rh(45.0f32.to_radians(), width as f32 / height as f32, 0.1, 100.0);
    let view_proj = projection * view;

    // Tile based rendering
    let tile_size = 32;
    let tiles_x = (width + tile_size - 1) / tile_size;
    let tiles_y = (height + tile_size - 1) / tile_size;
    let total_tiles = tiles_x * tiles_y;

    loop {
        if PANIC_STATE.load(Ordering::Acquire) { break; }

        let tile_idx = NEXT_TILE.fetch_add(1, Ordering::Relaxed);
        if tile_idx >= total_tiles {
            break; // Frame done
        }

        let ty = tile_idx / tiles_x;
        let tx = tile_idx % tiles_x;

        let start_x = tx * tile_size;
        let start_y = ty * tile_size;
        let end_x = (start_x + tile_size).min(width);
        let end_y = (start_y + tile_size).min(height);

        let tile_w = end_x - start_x;
        let tile_h = end_y - start_y;

        let mut buffer = Vec::with_capacity((tile_w * tile_h) as usize);

        for y in start_y..end_y {
            for x in start_x..end_x {
                // Ray generation
                let ndc_x = (x as f32 / width as f32) * 2.0 - 1.0;
                let ndc_y = 1.0 - (y as f32 / height as f32) * 2.0;

                let inv_vp = view_proj.inverse();
                let target_world = inv_vp.project_point3(vec3(ndc_x, ndc_y, 1.0));

                let ray_origin = camera_pos;
                let ray_dir = (target_world - camera_pos).normalize();

                let mut r: u8 = 100;
                let mut g: u8 = 149;
                let mut b: u8 = 237;

                let mut min_dist = f32::MAX;

                for (block_pos, block) in &ctx.world.blocks {
                    let min = *block_pos - vec3(0.5, 0.5, 0.5);
                    let max = *block_pos + vec3(0.5, 0.5, 0.5);

                    if let Some(dist) = ray_aabb_intersect(ray_origin, ray_dir, min, max) {
                        if dist > 0.0 && dist < min_dist {
                            min_dist = dist;
                            if block.id == 1 {
                                r = 100; g = 100; b = 100;
                            } else {
                                r = 200; g = 50; b = 50;
                            }

                            // Simple lighting
                            let hit_point = ray_origin + ray_dir * dist;
                            let center = *block_pos;
                            let local_hit = hit_point - center;
                            let abs_hit = local_hit.abs();

                            let mut brightness = 1.0;
                            if abs_hit.x > abs_hit.y && abs_hit.x > abs_hit.z { brightness = 0.8; }
                            else if abs_hit.y > abs_hit.x && abs_hit.y > abs_hit.z { brightness = 1.0; }
                            else { brightness = 0.6; }

                            r = (r as f32 * brightness) as u8;
                            g = (g as f32 * brightness) as u8;
                            b = (b as f32 * brightness) as u8;
                        }
                    }
                }
                buffer.push(BltPixel::new(r, g, b));
            }
        }

        // Draw tile
        while DRAW_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            spin_loop();
        }

        let _ = ctx.scr.gop.blt(uefi::proto::console::gop::BltOp::BufferToVideo {
            buffer: &buffer,
            src: BltRegion::Full,
            dest: (start_x, start_y),
            dims: (tile_w, tile_h),
        });

        DRAW_LOCK.store(false, Ordering::Release);
    }

    // Wait loop
    loop {
        if PANIC_STATE.load(Ordering::Acquire) { break; }
        spin_loop()
    }
    OK
}

fn ray_aabb_intersect(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let t1 = (min.x - origin.x) / dir.x;
    let t2 = (max.x - origin.x) / dir.x;
    let t3 = (min.y - origin.y) / dir.y;
    let t4 = (max.y - origin.y) / dir.y;
    let t5 = (min.z - origin.z) / dir.z;
    let t6 = (max.z - origin.z) / dir.z;

    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if tmax < 0.0 {
        return None;
    }

    if tmin > tmax {
        return None;
    }

    Some(tmin)
}

/// 之后再写 只访问全局变量
pub extern "efiapi" fn _game_task_safe(arg: *mut c_void) {
    if arg.is_null() { return }
    let _ = _run_safe();
    todo!()
}

fn _run_safe() -> Result {
    todo!()
}
