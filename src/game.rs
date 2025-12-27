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
    pos: Vec3A::new(8.5, 9.5, 4.5),
    rot: Mat3A::IDENTITY,
};

#[repr(C)]
pub struct GameContext<'a> {
    pub mp: &'a MpServices,
    pub scr: &'a mut Screen,
    pub num_cores: usize,
}

/// efiapi 只能访问static全局静态变量 或者是 arg上下文参数
pub extern "efiapi" fn game_task(arg: *mut c_void) {
    if arg.is_null() { return; }
    // 不安全的获取参数上下文
    let ctx = unsafe { &mut *arg.cast::<GameContext>() };
    // 初始化失败一个核就设置全局错误状态,进入打印错误代码阶段
    if let Err(e) = run(ctx) {
        PANIC_STATE.store(true, Ordering::SeqCst);
        kernel_panic(&mut *ctx.scr, e)
    }
}

use crate::world;

pub fn run(ctx: &mut GameContext) -> Result {
    let id = t!(ctx.mp.who_am_i());

    if id == 0 {
        world::init_world();
        // 初始化摄像机向下看
        unsafe {
            PLAYER.rot = Mat3A::from_rotation_x(-0.2);
        }
    }


    loop {
        if PANIC_STATE.load(Ordering::SeqCst) { return OK; }

        if id == 0 {
            system::with_stdin(|input| {
                if let Ok(Some(key)) = input.read_key() {
                    // 不安全的可变静态变量访问
                    unsafe {
                        match key {
                            Key::Printable(wide_char) => match wide_char.into() {
                                'w' => PLAYER.pos += PLAYER.rot.z_axis * 0.1,
                                's' => PLAYER.pos -= PLAYER.rot.z_axis * 0.1,
                                'a' => PLAYER.pos -= PLAYER.rot.x_axis * 0.1,
                                'd' => PLAYER.pos += PLAYER.rot.x_axis * 0.1,
                                _ => {}
                            },
                            Key::Special(special_key) => match special_key {
                                ScanCode::UP => PLAYER.rot *= Mat3A::from_rotation_x(0.1),
                                ScanCode::DOWN => PLAYER.rot *= Mat3A::from_rotation_x(-0.1),
                                ScanCode::LEFT => PLAYER.rot *= Mat3A::from_rotation_y(0.1),
                                ScanCode::RIGHT => PLAYER.rot *= Mat3A::from_rotation_y(-0.1),
                                ScanCode::PAGE_UP => PLAYER.pos.y += 0.1,
                                ScanCode::PAGE_DOWN => PLAYER.pos.y -= 0.1,
                                _ => {}
                            },
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

        // 从共享静态变量中读取player状态
        let player = unsafe { &(*addr_of_mut!(PLAYER)) };
        ray_march(&player, start_y, end_y);

        if id == 0 {
            ctx.scr.draw_buffer()?
        }
    }
}
