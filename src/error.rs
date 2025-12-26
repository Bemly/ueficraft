use alloc::borrow::Cow;
use alloc::format;
use crate::graphics::Screen;

/// 返回结构的默认参数构造
pub type Result<T = (), E = Error> = core::result::Result<T, E>;
pub const OK: Result = Ok(());

/// 继承其他上层错误类型
/// From会"单步"隐式转换到枚举体内
#[derive(Debug)]
pub enum ErrorType {
    Uefi(uefi::Error),
    _Reserve,
}

/// 错误类型包装器
#[derive(Debug)]
pub struct Error {
    pub err: ErrorType,
    pub file: &'static str,
    pub line: u32,
    // Cow<'static, str> 可以是 &'static str 也可以是 String
    pub info: Option<Cow<'static, str>>
}

/// 抛出自定义错误
#[macro_export]
macro_rules! throw {
    // 无描述信息
    ($err:expr) => {
        return Err($crate::error::Error {
            err: $err.into(),
            file: file!(),
            line: line!(),
            info: None,
        })
    };

    // 单个静态字符串字面量
    ($err:expr, $msg:literal) => {
        return Err($crate::error::Error {
            err: $err.into(),
            file: file!(),
            line: line!(),
            info: Some(alloc::borrow::Cow::Borrowed($msg)),
        })
    };

    // 带有格式化参数
    ($err:expr, $($arg:tt)*) => {
        return Err($crate::error::Error {
            err: $err.into(),
            file: file!(),
            line: line!(),
            info: Some(alloc::borrow::Cow::Owned(alloc::format!($($arg)*))),
        })
    };
}

/// 返回值处理包装器
#[macro_export]
macro_rules! t {
    // 捕获第一个表达式，以及后面可能存在的任意数量的参数
    ($e:expr $(, $($arg:tt)*)?) => {
        match $e {
            Ok(val) => val,
            // throw!被macro_export提升到error模块外了
            Err(e) => $crate::throw!(e $(, $($arg)*)?),
        }
    };
}

impl From<uefi::Error> for ErrorType {
    fn from(e: uefi::Error) -> Self {
        ErrorType::Uefi(e)
    }
}

/// 捕获错误并打印,一把抓住屏幕即刻炼化
pub fn kernel_panic(mut scr: Screen, e: Error) {
    scr.println(&format!("Kernel panic at {}:{}\n{}", e.file, e.line, e.info.unwrap_or(Cow::Borrowed(""))));
    scr.println(&format!("Error: {:?}", e.err));
}