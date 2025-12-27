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
    // 出生在 (4.5, 3.0, 4.5)，确保在地面y=0之上
    pos: Vec3A::new(4.5, 3.0, 4.5),
    rot: Mat3A::IDENTITY,
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

// 玩家身高，用于碰撞检测
const PLAYER_HEIGHT: f32 = 1.8;
// 玩家眼睛高度在身高中的比例
const PLAYER_EYE_RATIO: f32 = 0.9;
// 玩家宽度，用于碰撞检测
const PLAYER_WIDTH: f32 = 0.6;


/// 检查给定位置是否会与方块发生碰撞
/// pos: 玩家眼睛的目标位置
fn is_colliding(pos: Vec3A) -> bool {
    let half_width = PLAYER_WIDTH / 2.0;
    let feet_y = pos.y - PLAYER_HEIGHT * PLAYER_EYE_RATIO;
    let head_y = pos.y + PLAYER_HEIGHT * (1.0 - PLAYER_EYE_RATIO);

    // 检查一个包围盒内的8个角点 + 中心点
    for dx in &[-half_width, half_width] {
        for dz in &[-half_width, half_width] {
            let check_x = pos.x + dx;
            let check_z = pos.z + dz;
            // 检查脚部、腰部、头部
            if world::get_block(check_x as i32, feet_y as i32, check_z as i32) > 0 ||
               world::get_block(check_x as i32, pos.y as i32, check_z as i32) > 0 ||
               world::get_block(check_x as i32, head_y as i32, check_z as i32) > 0 {
                return true;
            }
        }
    }
    false
}

pub fn run(ctx: &mut GameContext) -> Result {
    let id = t!(ctx.mp.who_am_i());

    if id == 0 {
        world::init_world();
        unsafe {
            // 初始化摄像机稍微向下看
            PLAYER.rot = Mat3A::from_rotation_x(0.2);
        }
    }


    loop {
        if PANIC_STATE.load(Ordering::SeqCst) { return OK; }

        if id == 0 {
            system::with_stdin(|input| {
                if let Ok(Some(key)) = input.read_key() {
                    unsafe {
                        let move_speed = 0.15;
                        let mut move_vec = Vec3A::ZERO;

                        match key {
                            Key::Printable(wide_char) => match wide_char.into() {
                                'w' | 'W' => move_vec += PLAYER.rot.z_axis,
                                's' | 'S' => move_vec -= PLAYER.rot.z_axis,
                                'a' | 'A' => move_vec -= PLAYER.rot.x_axis,
                                'd' | 'D' => move_vec += PLAYER.rot.x_axis,
                                _ => {}
                            },
                            Key::Special(special_key) => match special_key {
                                ScanCode::UP => PLAYER.rot *= Mat3A::from_rotation_x(0.1),
                                ScanCode::DOWN => PLAYER.rot *= Mat3A::from_rotation_x(-0.1),
                                ScanCode::LEFT => PLAYER.rot *= Mat3A::from_rotation_y(0.1),
                                ScanCode::RIGHT => PLAYER.rot *= Mat3A::from_rotation_y(-0.1),
                                ScanCode::PAGE_UP => move_vec.y += 1.0,
                                ScanCode::PAGE_DOWN => move_vec.y -= 1.0,
                                _ => {}
                            },
                        }

                        // 水平移动
                        let mut horizontal_move = Vec3A::new(move_vec.x, 0.0, move_vec.z);
                        if horizontal_move.length_squared() > 0.0 {
                            horizontal_move = horizontal_move.normalize() * move_speed;
                        }

                        // 垂直移动
                        let vertical_move = Vec3A::new(0.0, move_vec.y * move_speed, 0.0);

                        let new_pos = PLAYER.pos + horizontal_move + vertical_move;

                        if !is_colliding(new_pos) {
                            PLAYER.pos = new_pos;
                        } else if !is_colliding(PLAYER.pos + horizontal_move) {
                            PLAYER.pos += horizontal_move;
                        } else if !is_colliding(PLAYER.pos + vertical_move) {
                            PLAYER.pos += vertical_move;
                        }
                    }
                }
                OK
            })?;
        }

        let rows_per_core = crate::render::FB_H / ctx.num_cores;
        let start_y = id * rows_per_core;
        let end_y = if id == ctx.num_cores - 1 {
            crate::render::FB_H
        } else {
            (id + 1) * rows_per_core
        };

        let player = unsafe { &(*addr_of_mut!(PLAYER)) };
        ray_march(&player, start_y, end_y);

        if id == 0 {
            ctx.scr.draw_buffer()?
        }
    }
}