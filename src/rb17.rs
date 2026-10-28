//! The Rust Book, глава 17 — async для начинающих.
//! Рантайм: `crate::rt` (замена крейта `trpl`).
//! Эталон (не подсматривать без нужды): `reference/rb17.rs`.

use crate::rt;
use std::time::{Duration, Instant};

async fn echo(message: &str) -> String {
    println!("run: echo - {message}");
    format!("echo: {message}")
}

fn echo_desugared(message: &str) -> impl Future<Output = String> {
    async move {
        println!("run: echo_disugared - {message}");
        format!("echo: {message}")
    }
}

pub fn f1_1() {
    let f1 = echo("future");
    println!("Future was created, but not called");

    let f1_v = rt::block_on(f1);
    let f2_v = rt::block_on(echo_desugared("future"));

    println!("value: echo - {f1_v}");
    println!("value: echo_disugared - {f2_v}");

    assert_eq!(f1_v, f2_v);
}

async fn download(site: &str, delay: u64) -> String {
    println!("started: {site}");
    rt::sleep(Duration::from_millis(delay)).await;
    println!("end: {site}");
    site.to_owned()
}

pub fn f1_2() {
    rt::block_on(async {
        let start = Instant::now();
        download("rust-lang.org", 80).await;
        println!("mid: {0:?}", start.elapsed());
        download("docs.rs", 30).await;
        println!("end: {0:?}", start.elapsed());
    });
}

pub fn f1_3() {
    rt::block_on(async {
        let start = Instant::now();
        let result = rt::select(download("rust-lang.org", 80), download("docs.rs", 30)).await;
        println!("result: {0:?} - {1:?}", result, start.elapsed());
    });
}
