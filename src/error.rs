use alloc::borrow::Cow;
use core::time::Duration;
use uefi::boot;
use crate::Screen;

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
        return core::result::Result::Err($crate::error::Error {
            err: $err.into(),
            file: core::file!(),
            line: core::line!(),
            info: core::option::Option::None,
        })
    };

    // 单个静态字符串字面量
    ($err:expr, $msg:literal) => {
        return core::result::Result::Err($crate::error::Error {
            err: $err.into(),
            file: core::file!(),
            line: core::line!(),
            info: core::option::Option::Some(alloc::borrow::Cow::Borrowed($msg)),
        })
    };

    // 带有格式化参数
    ($err:expr, $($arg:tt)+) => {
        return core::result::Result::Err($crate::error::Error {
            err: $err.into(),
            file: core::file!(),
            line: core::line!(),
            info: core::option::Option::Some(alloc::borrow::Cow::Owned(alloc::format!($($arg)*))),
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

/// 捕获错误并打印 LSP识别不了宏对模块的使用（恼
#[allow(unused_variables, unused_imports)]
pub fn kernel_panic(scr:&mut Screen, e: Error) -> ! {
    const SHUTDOWN_COUNTDOWN_MIN: u64 = 1;

    use alloc::format;
    macro_rules! println {
        ($($arg:tt)*) => {{
            let _ = scr.println(&alloc::format!($($arg)*));
        }} // 打印出错我也不管了
    }

    println!("Kernel panic at {}:{}\n{:?}", e.file, e.line, e.info);
    match e.err {
        ErrorType::Uefi(e) => println!("UEFI error: {}", e),
        _ => println!("Error: {:?}", e.err),
    }

    println!("Kernel will shutdown in {} minute(s).", SHUTDOWN_COUNTDOWN_MIN);
    boot::stall(Duration::from_mins(SHUTDOWN_COUNTDOWN_MIN));
    panic!("Kernel Panic!");
}