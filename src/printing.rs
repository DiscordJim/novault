use colorize::AnsiColor;



pub enum LogType {
    Info,
    Error
}

#[macro_export]
macro_rules! console_log {
    ($lt:ident,  $($rest:tt)*) => {
        crate::printing::_print_log(crate::printing::LogType::$lt);
        println!($($rest)*);
    };
}

pub fn _print_log(lt: LogType, ) {
    match lt {
        LogType::Info => print!("{} ", "INFO".green().bold()),
        LogType::Error => print!("{} ", "ERROR".red().bold())
    }
}