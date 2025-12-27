use core::f32::consts::FRAC_PI_2;
use core::ffi::c_void;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive, ScopedProtocol};
use uefi::prelude::system;
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::Key;
use uefi::proto::pi::mp::MpServices;
use uefi::ResultExt;
use crate::error::{kernel_panic, OK, Result};
use glam::{Mat3A, Vec3A};
use crate::render::{Player, ray_march, Screen};
use crate::t;

static PANIC_STATE: AtomicBool = AtomicBool::new(false);

static mut PLAYER: Player = Player {
    pos: Vec3A::new(4.5, 3.0, 4.5),
    velocity: Vec3A::ZERO,
    yaw: 0.0,
    pitch: 0.0,
    is_crouching: false,
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

// 游戏参数
const PLAYER_HEIGHT: f32 = 1.8;
const CROUCH_HEIGHT: f32 = 1.3;
const PLAYER_EYE_RATIO: f32 = 0.9;
const CROUCH_EYE_RATIO: f32 = 0.8;
const PLAYER_WIDTH: f32 = 0.6;
const GRAVITY: f32 = 0.025;
const JUMP_STRENGTH: f32 = 0.3;
const FLY_SPEED: f32 = 0.15;
const MOVE_SPEED: f32 = 0.1;
const MOUSE_SENSITIVITY: f32 = 0.002;

fn get_player_height(is_crouching: bool) -> f32 {
    if is_crouching { CROUCH_HEIGHT } else { PLAYER_HEIGHT }
}

fn get_eye_ratio(is_crouching: bool) -> f32 {
    if is_crouching { CROUCH_EYE_RATIO } else { PLAYER_EYE_RATIO }
}

fn is_colliding_at(pos: Vec3A, is_crouching: bool) -> bool {
    let player_height = get_player_height(is_crouching);
    let eye_ratio = get_eye_ratio(is_crouching);
    let half_width = PLAYER_WIDTH / 2.0;

    let player_min_y = pos.y - player_height * eye_ratio;
    let player_max_y = pos.y + player_height * (1.0 - eye_ratio);

    let min_bx = (pos.x - half_width) as i32;
    let max_bx = (pos.x + half_width) as i32;
    let min_by = player_min_y as i32;
    let max_by = player_max_y as i32;
    let min_bz = (pos.z - half_width) as i32;
    let max_bz = (pos.z + half_width) as i32;
    
    for by in min_by..=max_by {
        for bz in min_bz..=max_bz {
            for bx in min_bx..=max_bx {
                if world::get_block(bx, by, bz) > 0 { return true; }
            }
        }
    }
    false
}

fn is_on_ground(pos: Vec3A, is_crouching: bool) -> bool {
    let check_pos = Vec3A::new(pos.x, pos.y - 0.05, pos.z);
    is_colliding_at(check_pos, is_crouching)
}

pub fn run(ctx: &mut GameContext) -> Result {
    let id = t!(ctx.mp.who_am_i());

    let mut pointer: Option<ScopedProtocol<Pointer>> = None;
    if id == 0 {
        world::init_world();
        if let Ok(handle) = get_handle_for_protocol::<Pointer>() {
            if let Ok(mut p) = open_protocol_exclusive::<Pointer>(handle) {
                let _ = p.reset(false);
                pointer = Some(p);
            }
        }
    }

    loop {
        if PANIC_STATE.load(Ordering::SeqCst) { return OK; }

        if id == 0 {
            unsafe {
                if let Some(p) = pointer.as_mut() {
                    if let Ok(Some(state)) = p.read_state() {
                        PLAYER.yaw -= state.relative_movement_x as f32 * MOUSE_SENSITIVITY;
                        PLAYER.pitch -= state.relative_movement_y as f32 * MOUSE_SENSITIVITY;
                        PLAYER.pitch = PLAYER.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
                    }
                }

                let mut wish_dir = Vec3A::ZERO;
                let mut flying_y = 0.0;
                let mut is_flying_cmd = false;
                let on_ground = is_on_ground(PLAYER.pos, PLAYER.is_crouching);
                
                system::with_stdin(|input| {
                    if let Ok(Some(key)) = input.read_key() {
                        let rot = Mat3A::from_rotation_y(PLAYER.yaw);
                        match key {
                            Key::Printable(c) => match c.into() {
                                'w'|'W' => wish_dir += rot.z_axis,
                                's'|'S' => wish_dir -= rot.z_axis,
                                'a'|'A' => wish_dir -= rot.x_axis,
                                'd'|'D' => wish_dir += rot.x_axis,
                                ' ' => if on_ground { PLAYER.velocity.y = JUMP_STRENGTH; } else { flying_y = FLY_SPEED; is_flying_cmd = true;},
                                'c'|'C' => if on_ground { PLAYER.is_crouching = !PLAYER.is_crouching; } else { flying_y = -FLY_SPEED; is_flying_cmd = true; },
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                    OK
                })?;

                // Horizontal movement
                let wish_dir_normalized = wish_dir.normalize_or_zero();
                PLAYER.velocity.x = wish_dir_normalized.x * MOVE_SPEED;
                PLAYER.velocity.z = wish_dir_normalized.z * MOVE_SPEED;

                // Vertical movement
                if is_flying_cmd {
                    PLAYER.velocity.y = flying_y;
                } else if on_ground {
                    if PLAYER.velocity.y < 0.0 { PLAYER.velocity.y = 0.0; }
                } else {
                    PLAYER.velocity.y -= GRAVITY;
                }

                // Collision and position update
                let mut new_pos = PLAYER.pos;
                let is_crouching = PLAYER.is_crouching;
                
                new_pos.x += PLAYER.velocity.x;
                if is_colliding_at(new_pos, is_crouching) { new_pos.x = PLAYER.pos.x; }
                
                new_pos.z += PLAYER.velocity.z;
                if is_colliding_at(new_pos, is_crouching) { new_pos.z = PLAYER.pos.z; }

                new_pos.y += PLAYER.velocity.y;
                if is_colliding_at(new_pos, is_crouching) {
                    new_pos.y = PLAYER.pos.y;
                    PLAYER.velocity.y = 0.0;
                }
                
                PLAYER.pos = new_pos;
            }
        }
        
        let rows_per_core = crate::render::FB_H / ctx.num_cores;
        let start_y = id * rows_per_core;
        let end_y = if id == ctx.num_cores - 1 { crate::render::FB_H } else { (id + 1) * rows_per_core };

        let player_copy = unsafe { *addr_of_mut!(PLAYER) };
        ray_march(&player_copy, start_y, end_y);

        if id == 0 {
            ctx.scr.draw_buffer()?;
        }
    }
}
