use core::ffi::c_void;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use glam::{IVec3, Mat4, Vec3};
use uefi::proto::pi::mp::MpServices;
use uefi_input2::init_keyboards_protocol;
use crate::error::{kernel_panic, OK, Result};
use crate::render::Screen;
use crate::t;
use crate::world::{Chunk, Block};

static PANIC_STATE: AtomicBool = AtomicBool::new(false);
static FRAME_BARRIER: AtomicUsize = AtomicUsize::new(0);
static CORES_FINISHED: AtomicUsize = AtomicUsize::new(0);

static mut FACES_PTR: *const crate::world::Face = core::ptr::null();
static mut FACES_LEN: usize = 0;
static mut VIEW_PROJ: Mat4 = Mat4::IDENTITY;

#[repr(C)]
pub struct GameContext<'bemly_> {
    pub mp: &'bemly_ MpServices,
    pub scr: &'bemly_ mut Screen,
    pub num_cores: usize,
}

pub extern "efiapi" fn game_task(arg: *mut c_void) {
    if arg.is_null() { return; }
    let ctx = unsafe { &mut *arg.cast::<GameContext>() };
    if let Err(e) = run(ctx) {
        if PANIC_STATE.compare_exchange(
            false, true, Ordering::AcqRel, Ordering::Acquire
        ).is_ok() {
            kernel_panic(&mut ctx.scr, e)
        }
    }
}

pub fn run(ctx: &mut GameContext) -> Result {
    let core_id = t!(ctx.mp.who_am_i());
    let num_cores = ctx.num_cores;

    if core_id == 0 {
        // Init world
        let mut chunk = Chunk::new(IVec3::ZERO);
        for x in 0..32 {
            for z in 0..32 {
                chunk.set(x, 0, z, Block::Bedrock);
                chunk.set(x, 1, z, Block::Dirt);
                chunk.set(x, 2, z, Block::Grass);
            }
        }
        chunk.set(8, 3, 8, Block::Stone);
        chunk.compress();

        let faces = chunk.generate_mesh(0);

        unsafe {
            FACES_PTR = faces.as_ptr();
            FACES_LEN = faces.len();
        }

        let mut camera_pos = Vec3::new(16.0, 20.0, 40.0);
        let camera_target = Vec3::new(16.0, 0.0, 16.0);
        let up = Vec3::Y;

        let aspect = ctx.scr.width as f32 / ctx.scr.height as f32;
        let projection = Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 1000.0);

        let mut frame_idx = 1;

        loop {
            if PANIC_STATE.load(Ordering::Acquire) { break; }

            // Update logic
            // Simple pan
            let offset = (frame_idx % 1000) as f32 * 0.05;
            camera_pos.x = 16.0 + offset;

            let view = Mat4::look_at_rh(camera_pos, camera_target, up);
            let view_proj = projection * view;

            unsafe { VIEW_PROJ = view_proj; }

            // Signal start
            FRAME_BARRIER.store(frame_idx, Ordering::Release);

            // Render own tiles
            render_tiles(ctx.scr, core_id, num_cores);

            // Wait for workers
            if num_cores > 1 {
                while CORES_FINISHED.load(Ordering::Acquire) < num_cores - 1 {
                    spin_loop();
                }
            }

            // Flush
            ctx.scr.flush()?;

            // Reset
            CORES_FINISHED.store(0, Ordering::Release);
            frame_idx += 1;
        }
    } else {
        let mut last_frame = 0;
        loop {
            if PANIC_STATE.load(Ordering::Acquire) { break; }

            let curr_frame = FRAME_BARRIER.load(Ordering::Acquire);
            if curr_frame > last_frame {
                render_tiles(ctx.scr, core_id, num_cores);
                last_frame = curr_frame;
                CORES_FINISHED.fetch_add(1, Ordering::Release);
            } else {
                spin_loop();
            }
        }
    }
    OK
}

fn render_tiles(scr: &Screen, core_id: usize, num_cores: usize) {
    let faces = unsafe { core::slice::from_raw_parts(FACES_PTR, FACES_LEN) };
    let view_proj = unsafe { VIEW_PROJ };

    let height = scr.height;
    let chunk_h = (height + num_cores - 1) / num_cores;
    let min_y = core_id * chunk_h;
    let max_y = core::cmp::min((core_id + 1) * chunk_h, height);

    if min_y >= max_y { return; }

    let tile = (0, min_y, scr.width, max_y);

    scr.clear_tile(tile);
    scr.draw_tile(faces, view_proj, tile);
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
