//! The Rust Book, глава 17 — async для начинающих.
//! Рантайм: `crate::rt` (замена крейта `trpl`).
//! Эталон (не подсматривать без нужды): `reference/rb17.rs`.

use crate::rt;
use std::{
    clone,
    time::{Duration, Instant},
};

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

fn add_desugared(a: i32, b: i32) -> impl Future<Output = i32> {
    async move {
        rt::sleep(Duration::from_millis(60)).await;
        a + b
    }
}

async fn add(a: i32, b: i32) -> i32 {
    add_desugared(a, b).await
}

pub fn e1_1() {
    rt::block_on(async {
        let a = add(2, 3).await;
        let b = add_desugared(2, 3).await;

        assert_eq!(a, b);
    });
}

// pub fn e1_2() {
//     download("rust-lang.org", 10);
// }

pub fn e1_3() {
    rt::block_on(async {
        match rt::select(download("rust-lang.org", 40), download("docs.rs", 60)).await {
            rt::Either::Left(v) => println!("Left: {v}"),
            rt::Either::Right(v) => println!("Right: {v}"),
        }
    });
}

async fn titles(sites: &[(&str, u64)]) -> Vec<String> {
    let start = Instant::now();
    let mut res: Vec<String> = Vec::with_capacity(sites.len());
    println!("start ****");
    for (site, delay) in sites {
        res.push(download(site, *delay).await);
    }
    println!("end: {:?} ms", start.elapsed().as_millis());
    res
}

pub fn e1_4() {
    rt::block_on(async {
        let list = titles(&[("rust-lang.org", 40), ("docs.rs", 60)]).await;
        println!("{list:?}");
    });
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

pub fn f2_2() {
    rt::block_on(async {
        let h = rt::spawn_task(async {
            for i in 0..10 {
                println!("Hi {i} first task");
                rt::sleep(ms(20)).await;
            }
        });

        for i in 0..5 {
            println!("Hi {i} second task");
            rt::sleep(ms(25)).await;
        }

        h.await;
    });
}

pub fn f2_3() {
    let fut1 = async {
        for i in 0..5 {
            println!("Hi {i} first task");
            rt::sleep(ms(20)).await;
        }
    };

    let fut2 = async {
        for i in 0..3 {
            println!("Hi {i} second task");
            rt::sleep(ms(40)).await;
        }
    };

    rt::block_on(async {
        rt::join(fut1, fut2).await;
    });
}

pub fn f2_4() {
    rt::block_on(async {
        let (tx, mut rx) = rt::channel();

        rt::spawn_task(async move {
            for v in ["hi", "from", "the", "future"] {
                rt::sleep(ms(15)).await;
                tx.send(v).unwrap();
            }
        });

        let start = Instant::now();
        while let Some(v) = rx.recv().await {
            println!("[{0} ms] receive \"{1}\"", start.elapsed().as_millis(), v);
        }
    });
}

pub fn f2_5() {
    rt::block_on(async {
        let (tx, mut rx) = rt::channel();

        let sender = rt::spawn_task(async move {
            for v in ["hi", "from", "the", "future"] {
                rt::sleep(ms(350)).await;
                tx.send(v).unwrap();
            }
        });

        let receiver = rt::spawn_task(async move {
            let start = Instant::now();
            while let Some(v) = rx.recv().await {
                println!("[{0} ms] receive \"{1}\"", start.elapsed().as_millis(), v);
            }
        });

        rt::join(sender, receiver).await;
    });
}

pub fn f2_6() {
    rt::block_on(async {
        let (tx1, mut rx) = rt::channel();
        let tx2 = tx1.clone();

        let sender1 = rt::spawn_task(async move {
            for v in ["hi", "from", "the", "future"] {
                rt::sleep(ms(150)).await;
                tx1.send(v).unwrap();
            }
        });

        let sender2 = rt::spawn_task(async move {
            for v in ["more", "messages", "for", "you"] {
                rt::sleep(ms(350)).await;
                tx2.send(v).unwrap();
            }
        });

        let receiver = rt::spawn_task(async move {
            let start = Instant::now();
            while let Some(v) = rx.recv().await {
                println!("[{0} ms] receive \"{1}\"", start.elapsed().as_millis(), v);
            }
        });

        rt::join!(sender1, sender2, receiver);
    });
}
