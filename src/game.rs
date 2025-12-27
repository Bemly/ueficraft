use core::f32::consts::FRAC_PI_2;
use core::ffi::c_void;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive, ScopedProtocol};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::ScanCode;
use uefi::proto::pi::mp::MpServices;
use crate::error::{kernel_panic, OK, Result};
use glam::{Mat3A, Vec3A};
use uefi_raw::Status;
use crate::render::{FB_H, Player, ray_march, Screen, FB_W};
use crate::simple_text_input_ex::{KeyData, SimpleTextInputExProtocol, LEFT_SHIFT_PRESSED, RIGHT_SHIFT_PRESSED};
use crate::t;
use crate::world;

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

// --- Physics --- 
const CROUCH_HEIGHT: f32 = 1.3;
const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_WIDTH: f32 = 0.6;
const GRAVITY: f32 = 0.025;
const JUMP_STRENGTH: f32 = 0.3;
const FLY_SPEED: f32 = 0.15;
const MOVE_SPEED: f32 = 0.1;
const MOUSE_SENSITIVITY: f32 = 0.002;

fn get_player_height(is_crouching: bool) -> f32 {
    if is_crouching { CROUCH_HEIGHT } else { PLAYER_HEIGHT }
}

fn is_colliding_at(pos: Vec3A, is_crouching: bool) -> bool {
    let height = get_player_height(is_crouching);
    let eye_ratio = if is_crouching { 0.8 } else { 0.9 };
    let half_w = PLAYER_WIDTH / 2.0;

    let min_y = pos.y - height * eye_ratio;
    let max_y = pos.y + height * (1.0 - eye_ratio);

    for by in (min_y as i32)..=(max_y as i32) {
        for bz in ((pos.z - half_w) as i32)..=((pos.z + half_w) as i32) {
            for bx in ((pos.x - half_w) as i32)..=((pos.x + half_w) as i32) {
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

pub extern "efiapi" fn game_task(arg: *mut c_void) {
    if arg.is_null() { return; }
    let ctx = unsafe { &mut *arg.cast::<GameContext>() };
    if let Err(e) = run(ctx) {
        PANIC_STATE.store(true, Ordering::SeqCst);
        kernel_panic(&mut ctx.scr, e)
    }
}

pub fn run(ctx: &mut GameContext) -> Result {
    let core_id = t!(ctx.mp.who_am_i());

    let mode = ctx.scr.gop.current_mode_info();
    let (width, height) = mode.resolution();
    let stride = mode.stride();
    let pixel_format = mode.pixel_format();

    let mut fb_ptr = ctx.scr.gop.frame_buffer();

    let chunk_size = height / ctx.num_cores;
    let start_y = core_id * chunk_size;
    let end_y = if core_id == ctx.num_cores - 1 { height } else { start_y + chunk_size };
    let player_copy = unsafe { *addr_of_mut!(PLAYER) };

    if core_id == 0 {
        world::init_world();

        let pointer_handle = t!(get_handle_for_protocol::<Pointer>());
        let mut pointer = t!(open_protocol_exclusive::<Pointer>(pointer_handle));

        let input_handle = t!(get_handle_for_protocol::<SimpleTextInputExProtocol>());
        let mut input =
            t!(open_protocol_exclusive::<SimpleTextInputExProtocol>(input_handle));

        loop {
            if PANIC_STATE.load(Ordering::SeqCst) { break; }

            unsafe {
                // --- Input ---
                // Pointer 逻辑保持不变，uefi-rs 已经包装好了
                if let Ok(Some(s)) = pointer.read_state() {
                    PLAYER.yaw -= s.relative_movement[0] as f32 * MOUSE_SENSITIVITY;
                    PLAYER.pitch -= s.relative_movement[1] as f32 * MOUSE_SENSITIVITY;
                    PLAYER.pitch = PLAYER.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
                }

                let mut wish_dir = Vec3A::ZERO;
                let mut jump_request = false;

                // 获取原始协议指针进行 FFI 调用
                let input_ex = &mut *input;
                let mut key_data = KeyData::default();

                // 使用手写的 read_key_stroke_ex 函数指针调用
                // 第一个参数必须是 self 指针 (input_ex as *mut _)
                while (input_ex.read_key_stroke_ex)(input_ex as *mut _, &mut key_data) == Status::SUCCESS {
                    let rot_y = Mat3A::from_rotation_y(PLAYER.yaw);

                    // 将 Char16 转为 u16 方便匹配
                    let char_code = u16::from(key_data.key.unicode_char);

                    // 检查 Shift 状态（可选：比如按住 Shift 跑得更快）
                    let is_shifting = (key_data.key_state.key_shift_state & (LEFT_SHIFT_PRESSED | RIGHT_SHIFT_PRESSED)) != 0;
                    let current_move_speed = if is_shifting { MOVE_SPEED * 1.5 } else { MOVE_SPEED };

                    match (key_data.key.scan_code, char_code) {
                        // W/A/S/D
                        (_, u) if matches!(u as u8, b'w' | b'W') => wish_dir += rot_y.z_axis,
                        (_, u) if matches!(u as u8, b's' | b'S') => wish_dir -= rot_y.z_axis,
                        (_, u) if matches!(u as u8, b'a' | b'A') => wish_dir -= rot_y.x_axis,
                        (_, u) if matches!(u as u8, b'd' | b'D') => wish_dir += rot_y.x_axis,

                        // Space
                        (_, u) if u as u8 == b' ' => jump_request = true,

                        // 方向键左(3) 和 右(4) 切换潜行 (UEFI ScanCode: Left=3, Right=4)
                        (3, _) | (4, _) => {
                            PLAYER.is_crouching = !PLAYER.is_crouching;
                        }

                        // ESC 键退出 (UEFI ScanCode: 23)
                        (23, _) => {
                            PANIC_STATE.store(true, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }

                // --- Physics & Collision ---
                // (这部分逻辑保持不变，但可以使用上面算出的 current_move_speed)
                let on_ground = is_on_ground(PLAYER.pos, PLAYER.is_crouching);

                let norm_wish = wish_dir.normalize_or_zero();
                PLAYER.velocity.x = norm_wish.x * MOVE_SPEED;
                PLAYER.velocity.z = norm_wish.z * MOVE_SPEED;

                // ... (后面省略的物理和渲染代码保持原样) ...

                // --- Physics 垂直部分 ---
                if jump_request && !on_ground {
                    PLAYER.velocity.y = FLY_SPEED;
                } else if PLAYER.is_crouching && !on_ground {
                    PLAYER.velocity.y = -FLY_SPEED;
                } else if on_ground {
                    PLAYER.velocity.y = if jump_request { JUMP_STRENGTH } else { 0.0 };
                } else {
                    PLAYER.velocity.y -= GRAVITY;
                }

                // --- Collision 移动应用 ---
                let mut new_pos = PLAYER.pos;
                new_pos.x += PLAYER.velocity.x;
                if is_colliding_at(new_pos, PLAYER.is_crouching) { new_pos.x = PLAYER.pos.x; }
                new_pos.z += PLAYER.velocity.z;
                if is_colliding_at(new_pos, PLAYER.is_crouching) { new_pos.z = PLAYER.pos.z; }
                new_pos.y += PLAYER.velocity.y;
                if is_colliding_at(new_pos, PLAYER.is_crouching) {
                    new_pos.y = PLAYER.pos.y;
                    PLAYER.velocity.y = 0.0;
                }
                PLAYER.pos = new_pos;
            }


        }
    } else {
        loop {
            if PANIC_STATE.load(Ordering::SeqCst) { break; }

            ray_march(
                &player_copy,
                0,
                FB_H,
                &mut fb_ptr, // 传入原始指针或包装好的 FrameBuffer
                stride,
                (FB_W, FB_H),
                pixel_format
            );
        }
    }
    OK
}