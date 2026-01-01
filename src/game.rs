use core::ffi::c_void;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};
use uefi::proto::pi::mp::MpServices;
use crate::error::{kernel_panic, OK, Result};
use crate::render::Screen;
use crate::t;

static PANIC_STATE: AtomicBool = AtomicBool::new(false);

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
        // 保证崩溃状态原子性 Acquire-AcqRelease-Acquire 双向屏障
        if PANIC_STATE.compare_exchange(
            false, true, Ordering::AcqRel, Ordering::Acquire
        ).is_ok() {
            kernel_panic(&mut ctx.scr, e)
        }
    }
}

pub fn run(ctx: &mut GameContext) -> Result {
    let core_id = t!(ctx.mp.who_am_i());

    if core_id == 0 {

        loop {
            if PANIC_STATE.load(Ordering::Acquire) { break; }
            spin_loop()

        }
    } else {
        loop {
            if PANIC_STATE.load(Ordering::Acquire) { break; }
            spin_loop()
        }
    }
    OK
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