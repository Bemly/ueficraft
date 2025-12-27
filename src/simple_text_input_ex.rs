use core::ffi::c_void;
use uefi::proto::unsafe_protocol;
use uefi_raw::{guid, Boolean, Event, Guid, Status};
use uefi_raw::protocol::console::InputKey;

pub type KeyToggleState = u8;
/// 键盘状态：包含 Shift/Alt/Ctrl 状态以及大写锁定等标志位
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyState {
    /// 这里的位掩码通过下面的 SHIFT_STATE 常量进行判断
    pub key_shift_state: u32,
    /// 这里的位掩码通过下面的 TOGGLE_STATE 常量进行判断
    pub key_toggle_state: KeyToggleState,
}

/// 完整的按键数据结构
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyData {
    pub key: InputKey,
    pub key_state: KeyState,
}

// --- 协议常量定义 ---

// 组合键 状态掩码
pub const SHIFT_STATE_VALID: u32     = 0x8000_0000;
pub const RIGHT_SHIFT_PRESSED: u32   = 0x0000_0001;
pub const LEFT_SHIFT_PRESSED: u32    = 0x0000_0002;
pub const RIGHT_CONTROL_PRESSED: u32 = 0x0000_0004;
pub const LEFT_CONTROL_PRESSED: u32  = 0x0000_0008;
pub const RIGHT_ALT_PRESSED: u32     = 0x0000_0010;
pub const LEFT_ALT_PRESSED: u32      = 0x0000_0020;
pub const RIGHT_LOGO_PRESSED: u32    = 0x0000_0040;
pub const LEFT_LOGO_PRESSED: u32     = 0x0000_0080;
pub const MENU_KEY_PRESSED: u32      = 0x0000_0100;
pub const SYS_REQ_PRESSED: u32       = 0x0000_0200;

// Toggle 状态掩码
pub const TOGGLE_STATE_VALID: u8     = 0x80;
pub const KEY_STATE_EXPOSED: u8      = 0x40;
pub const SCROLL_LOCK_ACTIVE: u8     = 0x01;
pub const NUM_LOCK_ACTIVE: u8        = 0x02;
pub const CAPS_LOCK_ACTIVE: u8       = 0x04;

// --- 协议接口定义 ---

/// 按键通知回调函数类型
pub type KeyNotifyFunction = extern "efiapi" fn(key_data: *mut KeyData) -> Status;

/// EFI_SIMPLE_TEXT_INPUT_EX_PROTOCOL
/// 允许获取修饰键（Shift/Alt/Ctrl）状态的扩展输入协议
#[derive(Debug)]
#[repr(C)]
#[unsafe_protocol("dd9e7534-7762-4698-8c14-f58517a625aa")]
pub struct SimpleTextInputExProtocol {
    /// 重置输入设备硬件
    pub reset: extern "efiapi" fn(this: *mut Self, extended_verification: Boolean) -> Status,

    /// 读取按键数据（包含 KeyState）
    pub read_key_stroke_ex: extern "efiapi" fn(this: *mut Self, key_data: *mut KeyData) -> Status,

    /// 等待按键的事件
    pub wait_for_key_ex: Event,

    /// 设置键盘指示灯状态（如 CapsLock）
    pub set_state:
        extern "efiapi" fn(this: *mut Self, key_toggle_state: *mut KeyToggleState) -> Status,

    /// 注册一个按键通知函数，当特定键按下时触发
    pub register_key_notify: extern "efiapi" fn(
        this: *mut Self,
        key_data: *mut KeyData,
        key_notification_function: KeyNotifyFunction,
        notify_handle: *mut *mut c_void,
    ) -> Status,

    /// 注销按键通知
    pub unregister_key_notify:
        extern "efiapi" fn(this: *mut Self, notification_handle: *mut c_void) -> Status,
}

impl SimpleTextInputExProtocol {
    pub const GUID: Guid = guid!("dd9e7534-7762-4698-8c14-f58517a625aa");
}