pub fn f1_1() {
    let answer = 42;
    let p1 = &answer;
    let p2 = &answer;

    println!("{:p} == {:p}", p1, p2);
    println!("{:p} != {:p}", &p1, &p2)
}

pub fn f1_2() {
    let s1: &str = "Hello world";
    let s2: &str = "Hello world";

    println!("s1: {:p}, s2: {:p}", s1.as_ptr(), s2.as_ptr());
    let owned: String = s1.to_owned();
    println!("heap: {:p}", owned.as_ptr());
}

pub fn e1_1() {
    let a = [0u8; 16];

    println!(
        "&a: {:p}; &a[0]: {:p}; &a[15]: {:p}",
        a.as_ptr(),
        &a[0],
        &a[15]
    )
}

pub fn e2_2() {
    let mut v = vec![(4,), (5,), (6,)];
    let first = &v[0];
    println!("{first:?}");
    v.push((7,));
    let first = &v[0];
    println!("{first:?}")
}

static GREETING: &str = "hi";
pub fn f3_1() {
    let as_stack: [u64; 4] = [5, 6, 7, 8];
    let as_heap: Box<[i32]> = vec![5, 6, 7, 8].into_boxed_slice();

    println!("static: {:p}", GREETING.as_ptr());
    println!("heap: {:p}", as_heap.as_ptr());
    println!("stack: {:p}", &as_stack);
}

pub type TConfig = [u8; 4];
static DEFAULT: TConfig = [0u8; 4];
static mut CONFIG: Option<TConfig> = None;
pub unsafe fn e3_init(conf: &TConfig) {
    unsafe {
        let p = &raw mut CONFIG;
        if (*p).is_none() {
            (*p) = Some(*conf);
        }
    }
}
pub fn get_config() -> &'static TConfig {
    let p = &raw const CONFIG;
    unsafe {
        match &*p {
            Some(cfg) => cfg,
            None => &DEFAULT,
        }
    }
}

pub fn into_pieces(mut s: String) -> (String, String) {
    let mid = s
        .char_indices()
        .nth(s.chars().count() / 2)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let right = s.split_off(mid);

    (s, right)
}

pub fn e3_2() -> &'static mut Vec<i32> {
    let b = Box::new(Vec::new());

    Box::leak(b)
}

use std::cell::{Cell, RefCell};

pub struct Counter {
    hits: Cell<usize>,
}

impl Counter {
    pub fn new() -> Self {
        Self { hits: Cell::new(0) }
    }

    pub fn record(&self) {
        self.hits.set(self.hits.get() + 1);
    }

    pub fn get(&self) -> usize {
        self.hits.get()
    }
}

pub struct Fib {
    cache: RefCell<Vec<Option<u64>>>,
}

impl Fib {
    pub fn new(size: usize) -> Self {
        Self {
            cache: RefCell::new(vec![None; size]),
        }
    }

    pub fn get(&self, n: usize) -> u64 {
        if let Some(v) = self.cache.borrow()[n] {
            return v;
        }

        let value = match n {
            0 => 0,
            1 => 1,
            _ => self.get(n - 1) + self.get(n - 2),
        };

        self.cache.borrow_mut()[n] = Some(value);
        value
    }
}
