use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};
use uefi::proto::console::text::Key;
use uefi::proto::pi::mp::MpServices;
use uefi::{boot, system, ResultExt};
use crate::error::{kernel_panic, Result, OK};
use crate::graphics::Screen;
use crate::t;

static PANIC_STATE: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub struct MultiMCTask<'task> {
    pub mp: &'task MpServices,
    pub scr: &'task mut Screen,
    pub num_cores: usize,
}

/// efiapi 只能访问static全局静态变量 或者是 arg上下文参数
pub extern "efiapi" fn multimc_task(arg: *mut c_void) {
    if arg.is_null() { return }
    // 不安全的获取参数上下文
    let ctx = unsafe { &mut *arg.cast::<MultiMCTask>() };
    // 初始化失败一个核就设置全局错误状态,进入打印错误代码阶段
    if let Err(e) = run(ctx) {
        PANIC_STATE.store(true, Ordering::SeqCst);
        kernel_panic(&mut *ctx.scr, e)
    }
}

fn run(ctx:&mut MultiMCTask) -> Result {
    let id = t!(ctx.mp.who_am_i());

    loop {
        // 其他核发生错误了直接安全退出
        if PANIC_STATE.load(Ordering::SeqCst) { return OK; }

        if id == 0 {
            system::with_stdin(|input| {
                let mut events = [input.wait_for_key_event().unwrap()];
                t!(boot::wait_for_event(&mut events).discard_errdata());

                // Handle input
                if let Ok(Some(key)) = input.read_key() {
                    match key {
                        Key::Printable(wide_char) => match wide_char {
                            _ => {}
                        },
                        Key::Special(special_key) => match special_key {
                            _ => {},
                        },
                    }
                };

                OK
            })?;
        }
    }
}

/// 之后再写 只访问全局变量
pub extern "efiapi" fn multimc_task_safe(arg: *mut c_void) {
    if arg.is_null() { return }
    todo!()
}

fn run_safe() -> Result {
    todo!()
}