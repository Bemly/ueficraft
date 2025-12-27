use core::ffi::c_void;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};
use uefi::proto::console::text::{Key, ScanCode};
use uefi::proto::pi::mp::MpServices;
use uefi::{system, ResultExt};
use crate::error::{kernel_panic, Result, OK};
use glam::{Mat3A, Vec3A};
use crate::render::{Player, ray_march, Screen};
use crate::t;

static PANIC_STATE: AtomicBool = AtomicBool::new(false);

static mut PLAYER: Player = Player {
    pos: Vec3A::new(4.5, 1.0, 4.5),
    rot: Mat3A::IDENTITY,
    velocity: Vec3A::ZERO,
};

#[repr(C)]
pub struct GameContext<'a> {
    pub mp: &'a MpServices,
    pub scr: &'a mut Screen,
    pub num_cores: usize,
}

pub extern "efiapi" fn game_task(arg: *mut c_void) {
    if arg.is_null() { return; }
    let ctx = unsafe { &mut *arg.cast::<GameContext>() };
    if let Err(e) = run(ctx) {
        PANIC_STATE.store(true, Ordering::SeqCst);
        kernel_panic(&mut *ctx.scr, e)
    }
}

use crate::world;

const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_EYE_RATIO: f32 = 0.9;
const PLAYER_WIDTH: f32 = 0.6;
const GRAVITY: f32 = 0.02;
const JUMP_STRENGTH: f32 = 0.3;
const MOVE_SPEED: f32 = 0.1;

/// 精确的AABB（轴对齐包围盒）碰撞检测
fn is_colliding_at(pos: Vec3A) -> bool {
    let half_width = PLAYER_WIDTH / 2.0;
    let player_min_x = pos.x - half_width;
    let player_max_x = pos.x + half_width;
    let player_min_z = pos.z - half_width;
    let player_max_z = pos.z + half_width;
    let player_min_y = pos.y - PLAYER_HEIGHT * PLAYER_EYE_RATIO;
    let player_max_y = pos.y + PLAYER_HEIGHT * (1.0 - PLAYER_EYE_RATIO);

    // 遍历玩家包围盒可能接触到的所有方块
    for x in (player_min_x as i32)..=(player_max_x as i32) {
        for y in (player_min_y as i32)..=(player_max_y as i32) {
            for z in (player_min_z as i32)..=(player_max_z as i32) {
                if world::get_block(x, y, z) > 0 {
                    // 只要有一个方块与玩家的AABB相交，就认为发生碰撞
                    return true;
                }
            }
        }
    }
    false
}

/// 检查玩家是否站在地面上
fn is_on_ground(player_pos: Vec3A) -> bool {
    let half_width = PLAYER_WIDTH / 2.0;
    let feet_y = player_pos.y - PLAYER_HEIGHT * PLAYER_EYE_RATIO;
    let ground_check_y = (feet_y - 0.1) as i32; // 检查脚下稍低一点的位置

    // 检查玩家包围盒底部的四个角点
    let x1 = (player_pos.x - half_width) as i32;
    let z1 = (player_pos.z - half_width) as i32;
    let x2 = (player_pos.x + half_width) as i32;
    let z2 = (player_pos.z + half_width) as i32;

    world::get_block(x1, ground_check_y, z1) > 0 ||
    world::get_block(x2, ground_check_y, z1) > 0 ||
    world::get_block(x1, ground_check_y, z2) > 0 ||
    world::get_block(x2, ground_check_y, z2) > 0
}

pub fn run(ctx: &mut GameContext) -> Result {
    let id = t!(ctx.mp.who_am_i());

    if id == 0 {
        world::init_world();
        unsafe { PLAYER.rot = Mat3A::from_rotation_x(0.2); }
    }

    loop {
        if PANIC_STATE.load(Ordering::SeqCst) { return OK; }

        if id == 0 {
             system::with_stdin(|input| {
                if let Ok(Some(key)) = input.read_key() {
                    unsafe {
                        let mut wish_dir = Vec3A::ZERO;
                        match key {
                            Key::Printable(wide_char) => match wide_char.into() {
                                'w' | 'W' => wish_dir += PLAYER.rot.z_axis,
                                's' | 'S' => wish_dir -= PLAYER.rot.z_axis,
                                'a' | 'A' => wish_dir -= PLAYER.rot.x_axis,
                                'd' | 'D' => wish_dir += PLAYER.rot.x_axis,
                                ' ' => { // Space for jump
                                    if is_on_ground(PLAYER.pos) {
                                        PLAYER.velocity.y = JUMP_STRENGTH;
                                    }
                                }
                                _ => {}
                            },
                            Key::Special(special_key) => match special_key {
                                ScanCode::UP => PLAYER.rot *= Mat3A::from_rotation_x(0.05),
                                ScanCode::DOWN => PLAYER.rot *= Mat3A::from_rotation_x(-0.05),
                                ScanCode::LEFT => PLAYER.rot *= Mat3A::from_rotation_y(0.05),
                                ScanCode::RIGHT => PLAYER.rot *= Mat3A::from_rotation_y(-0.05),
                                ScanCode::PAGE_UP => PLAYER.pos.y += 0.2, // Noclip up
                                ScanCode::PAGE_DOWN => PLAYER.pos.y -= 0.2, // Noclip down
                                _ => {}
                            },
                        }

                        // --- Physics and Collision --- 
                        // 1. Apply gravity
                        PLAYER.velocity.y -= GRAVITY;

                        // 2. Update horizontal velocity based on input
                        let horizontal_wish = Vec3A::new(wish_dir.x, 0.0, wish_dir.z).normalize_or_zero();
                        PLAYER.velocity.x = horizontal_wish.x * MOVE_SPEED;
                        PLAYER.velocity.z = horizontal_wish.z * MOVE_SPEED;

                        // 3. Resolve collisions and move (slide along walls)
                        let mut new_pos = PLAYER.pos;
                        
                        // Move X
                        new_pos.x += PLAYER.velocity.x;
                        if is_colliding_at(new_pos) { 
                            new_pos.x = PLAYER.pos.x; 
                            PLAYER.velocity.x = 0.0;
                        }

                        // Move Z
                        new_pos.z += PLAYER.velocity.z;
                        if is_colliding_at(new_pos) { 
                            new_pos.z = PLAYER.pos.z; 
                            PLAYER.velocity.z = 0.0;
                        }

                        // Move Y
                        new_pos.y += PLAYER.velocity.y;
                        if is_colliding_at(new_pos) {
                            if PLAYER.velocity.y < 0.0 { // landed on something
                               // when landing, we might be slightly inside a block. Snap to top.
                               let feet_y = new_pos.y - PLAYER_HEIGHT * PLAYER_EYE_RATIO;
                               new_pos.y = ((feet_y as i32) + 1) as f32 + PLAYER_HEIGHT * PLAYER_EYE_RATIO;
                            }
                            PLAYER.velocity.y = 0.0;
                        }

                        PLAYER.pos = new_pos;
                    }
                }
                OK
            })?;
        }

        let rows_per_core = crate::render::FB_H / ctx.num_cores;
        let start_y = id * rows_per_core;
        let end_y = if id == ctx.num_cores - 1 { crate::render::FB_H } else { (id + 1) * rows_per_core };

        let player_copy = unsafe { *addr_of_mut!(PLAYER) };
        ray_march(&player_copy, start_y, end_y);

        if id == 0 {
            ctx.scr.draw_buffer()?
        }
    }
}