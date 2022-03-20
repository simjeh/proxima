
#[cfg(not(target_arch = "wasm32"))]
use termion::{style, color::Rgb, color};

/// Prints the given string with the given color.
///
/// ## Example
/// ```
/// use optima::utils::utils_console_output::{optima_print, PrintMode, PrintColor};
/// optima_print("test", PrintMode::Print, PrintColor::Blue, false);
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub fn optima_print(s: &str, mode: PrintMode, color: PrintColor, bolded: bool) {
    let mut string = "".to_string();
    if bolded { string += format!("{}", style::Bold).as_str() }
    if &color != &PrintColor::None {
        let c = color.get_color_triple();
        string += format!("{}", color::Fg(Rgb(c.0, c.1, c.2))).as_str();
    }
    string += s;
    string += format!("{}", style::Reset).as_str();
    match mode {
        PrintMode::Println => { println!("{}", string); }
        PrintMode::Print => { print!("{}", string); }
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

#[cfg(target_arch = "wasm32")]
#[warn(unused_variables)]
#[allow(unused)]
pub fn optima_print(s: &str, mode: PrintMode, color: PrintColor, bolded: bool) {
    println!("{}", s);
    log(s);
}


/// Enum that is used in print_termion_string function.
/// Println will cause a new line after each line, while Print will not.
#[derive(Clone, Debug)]
pub enum PrintMode {
    Println,
    Print
}

/// Defines color for an optima print command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrintColor {
    None,
    Blue,
    Green,
    Red,
    Yellow,
    Cyan,
    Magenta
}
#[cfg(not(target_arch = "wasm32"))]
impl PrintColor {
    pub fn get_color_triple(&self) -> (u8, u8, u8) {
        match self {
            PrintColor::None => { (0,0,0) }
            PrintColor::Blue => { return (0, 0, 255) }
            PrintColor::Green => { return (0, 255, 0) }
            PrintColor::Red => { return (255, 0, 0) }
            PrintColor::Yellow => { return (255, 255, 0) }
            PrintColor::Cyan => { return (0, 255, 255) }
            PrintColor::Magenta => { return (255, 0, 255) }
        }
    }
}

