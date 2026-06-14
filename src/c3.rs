use std::mem::{align_of, size_of};
use std::num::NonZeroU32;

#[repr(C)]
#[derive(Default)]
struct FooC {
    tiny: bool,
    normal: u32,
    smoll: u8,
    long: u128,
    short: u16,
}

#[derive(Default)]
struct FooRust {
    tiny: bool,
    normal: u32,
    smoll: u8,
    long: u128,
    short: u16,
}

pub fn compare_rept() -> (usize, usize, usize, usize) {
    (
        size_of::<FooC>(),
        align_of::<FooC>(),
        size_of::<FooRust>(),
        align_of::<FooRust>(),
    )
}

enum Shape {
    Circle { radius: f32 },
    Rect { w: f32, h: f32 },
}

pub fn f2_1() {
    println!("Shape: {}", size_of::<Shape>()); // 24
    println!("дискриминант + набивка: {}", size_of::<Shape>() - 8);
}

pub fn f2_2() {
    // Указатели не бывают нулевыми => Null годится как дискриминант None:
    assert_eq!(size_of::<Option<Box<u8>>>(), size_of::<Box<u8>>()); // 8 == 8
    assert_eq!(size_of::<Option<&u8>>(), size_of::<&u8>()); // 8 == 8
    println!(
        "size_of::<Option<Box<u8>>> = {}",
        size_of::<Option<Box<u8>>>()
    );

    assert_eq!(size_of::<Option<bool>>(), 1);
    println!("size_of::<Option<bool>> = {}", size_of::<Option<bool>>());

    assert_eq!(size_of::<Option<NonZeroU32>>(), 4);
    println!(
        "size_of::<Option<NonZeroU32>> = {}",
        size_of::<Option<NonZeroU32>>()
    );

    assert_eq!(size_of::<Option<u32>>(), 8);
    assert_eq!(size_of::<u32>(), 4);
    println!("size_of::<Option<u32>> = {}", size_of::<Option<u32>>());
    println!("size_of::<u32> = {}", size_of::<u32>());
}

pub fn e2_1() {
    println!("size_of::<u8> = {}", size_of::<u8>());
    println!("size_of::<Option<u8>> = {}", size_of::<Option<u8>>());
    println!(
        "size_of::<Option<Option<u8>>> = {}",
        size_of::<Option<Option<u8>>>()
    );
    println!(
        "size_of::<Option<Option<Option<u8>>> = {}",
        size_of::<Option<Option<Option<u8>>>>()
    );
}

pub fn e2_2() {
    println!(
        "size_of::<Result<u64, String>> = {}",
        size_of::<Result<u64, String>>()
    );
    println!(
        "size_of::<Result<u64, Vec<u8>>> = {}",
        size_of::<Result<u64, Vec<u8>>>()
    );
}

trait Draw {
    fn draw(&self) -> String;
}

impl Draw for i32 {
    fn draw(&self) -> String {
        format!("int [{self}]")
    }
}

pub fn f3_1() {
    const WORD: usize = size_of::<usize>();
    println!("WORD = [{WORD}]");
    assert_eq!(size_of::<&u8>(), WORD);
    assert_eq!(size_of::<Box<[u8; 4]>>(), WORD); // [u8; 4] — Sized!
    println!("size_of::<Box<[u8; 4]>> = [{}]", size_of::<Box<[u8; 4]>>());

    assert_eq!(size_of::<&[u8]>(), 2 * WORD); // указатель + длина
    println!("size_of::<&[u8]> = [{}]", size_of::<&[u8]>());
    assert_eq!(size_of::<&str>(), 2 * WORD); // str — тоже срез (байтов)
    println!("size_of::<&str> = [{}]", size_of::<&str>());
    assert_eq!(size_of::<&dyn Draw>(), 2 * WORD); // указатель + vtable
    println!("size_of::<&dyn Draw> = [{}]", size_of::<&dyn Draw>());
    assert_eq!(size_of::<Box<dyn Draw>>(), 2 * WORD);

    // Option<толстый указатель> — ниша работает и тут:
    assert_eq!(size_of::<Option<&[u8]>>(), 2 * WORD);
    println!(
        "size_of::<Option<&[u8]>> = [{}]",
        size_of::<Option<&[u8]>>()
    );
}

use std::fmt::Display;

fn show_sized<T: Display>(v: &T) -> String {
    format!("[{v}]")
}

fn show_any<T: Display + ?Sized>(v: &T) -> String {
    format!("[{v}]")
}

pub fn f3_3() {
    let s: &str = "hi";
    let d: &dyn Display = &42;

    // show_sized(s);
    // show_sized(d);

    println!("{}", show_any(s)); // [hi]
    println!("{}", show_any(d)); // [42]
    println!("{}", show_any(&7)); // [7] — Sized-типы тоже подходят

    println!("{}", show_sized(&"hi")); // [hi]
    println!("{}", show_sized(&7)); // [7]
}
