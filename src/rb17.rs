//! The Rust Book, глава 17 — async для начинающих.
//! Рантайм: `crate::rt` (замена крейта `trpl`).
//! Эталон (не подсматривать без нужды): `reference/rb17.rs`.

use crate::rt::{self, select};
use std::{
    pin::Pin,
    process::Output,
    thread,
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

async fn download(site: &str, delay: Duration) -> String {
    println!("started: {site}");
    rt::sleep(delay).await;
    println!("end: {site}");
    site.to_owned()
}

pub fn f1_2() {
    rt::block_on(async {
        let start = Instant::now();
        download("rust-lang.org", ms(80)).await;
        println!("mid: {0:?}", start.elapsed());
        download("docs.rs", ms(30)).await;
        println!("end: {0:?}", start.elapsed());
    });
}

pub fn f1_3() {
    rt::block_on(async {
        let start = Instant::now();
        let result = rt::select(
            download("rust-lang.org", ms(80)),
            download("docs.rs", ms(30)),
        )
        .await;
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
        match rt::select(
            download("rust-lang.org", ms(40)),
            download("docs.rs", ms(60)),
        )
        .await
        {
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
        res.push(download(site, ms(*delay)).await);
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
            45i32
        });

        for i in 0..5 {
            println!("Hi {i} second task");
            rt::sleep(ms(25)).await;
        }

        println!("{0:?}", h.await);
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

        rt::spawn_task(async move {
            for v in ["hi", "from", "the", "future"] {
                rt::sleep(ms(350)).await;
                tx.send(v).unwrap();
            }
        });

        let receiver = async {
            let start = Instant::now();
            while let Some(v) = rx.recv().await {
                println!("[{0} ms] receive \"{1}\"", start.elapsed().as_millis(), v);
            }
        };

        //rt::join(sender, receiver).await;
        //sender.await;
        receiver.await;
    });
}

pub fn f2_6() {
    rt::block_on(async {
        let (tx1, mut rx) = rt::channel::<String>();
        let tx2 = tx1.clone();
        let tx3 = tx1.clone();

        let sender1 = async move {
            for v in ["hi", "from", "the", "future"] {
                rt::sleep(ms(150)).await;
                tx1.send(v.into()).unwrap();
            }
        };

        let sender2 = async move {
            for v in ["more", "messages", "for", "you"] {
                rt::sleep(ms(350)).await;
                tx2.send(v.into()).unwrap();
            }
        };

        let sender3 = async move {
            for v in 1..=3 {
                rt::sleep(ms(100)).await;
                tx3.send(v.to_string()).unwrap();
            }
        };

        let receiver = async {
            let start = Instant::now();
            while let Some(v) = rx.recv().await {
                println!("[{0} ms] receive \"{1}\"", start.elapsed().as_millis(), v);
            }
        };

        rt::join!(sender1, sender2, sender3, receiver);
    });
}

pub fn e2_5() {
    rt::block_on(async {
        let (tx1, mut rx1) = rt::channel::<i32>();
        let (tx2, mut rx2) = rt::channel::<i32>();

        let h = rt::spawn_task(async move {
            while let Some(v) = rx1.recv().await {
                println!("rx1 received {v}");
                tx2.send(v + 1).unwrap();
            }
            println!("B: канал закрыт, выхожу");
        });

        tx1.send(0).unwrap();
        while let Some(v) = rx2.recv().await {
            println!("rx2 received {v}");
            if v >= 5 {
                break;
            }
            tx1.send(v).unwrap();
        }

        drop(tx1);
        h.await.unwrap();
    });
}

fn slow(name: &str, _ms: u64) {
    thread::sleep(ms(_ms));
    println!("'{name}' ran for {_ms} ms");
}

pub fn f3_1() {
    let start = Instant::now();

    let a = async {
        println!("'a' started");
        slow("a", 30);
        slow("a", 10);
        slow("a", 20);
        rt::sleep(ms(50)).await;
        println!("'a' finished.");
    };

    let b = async {
        println!("'b' started");
        slow("b", 75);
        slow("b", 10);
        slow("b", 15);
        slow("b", 100);
        rt::sleep(ms(50)).await;
        println!("'b' finished.");
    };

    rt::block_on(async move {
        rt::select(a, b).await;
    });

    println!("duration {}ms", start.elapsed().as_millis());
}

pub fn f3_2() {
    let start = Instant::now();

    let a = async {
        println!("'a' started");
        slow("a", 30);
        rt::yield_now().await;
        slow("a", 10);
        rt::yield_now().await;
        slow("a", 20);
        rt::yield_now().await;
        println!("'a' finished.");
    };

    let b = async {
        println!("'b' started");
        slow("b", 75);
        rt::yield_now().await;
        slow("b", 10);
        rt::yield_now().await;
        slow("b", 15);
        rt::yield_now().await;
        slow("b", 100);
        println!("'b' finished.");
    };

    rt::block_on(async move {
        rt::select(a, b).await;
    });

    println!("duration {}ms", start.elapsed().as_millis());
}

async fn timeout<F: Future>(fut: F, max: Duration) -> Result<F::Output, Duration> {
    match rt::select(fut, rt::sleep(max)).await {
        rt::Either::Left(v) => Ok(v),
        rt::Either::Right(_) => Err(max),
    }
}

pub fn f3_3() {
    rt::block_on(async {
        match timeout(
            async {
                rt::sleep(ms(300)).await;
                "Finally finished"
            },
            ms(100),
        )
        .await
        {
            Ok(mes) => println!("Succeeded with '{mes}'"),
            Err(d) => println!("Failed after '{}' ms", d.as_millis()),
        }

        match timeout(
            async {
                rt::sleep(ms(300)).await;
                "Finally finished"
            },
            ms(500),
        )
        .await
        {
            Ok(mes) => println!("Succeeded with '{mes}'"),
            Err(d) => println!("Failed after '{}' ms", d.as_millis()),
        }
    });
}

async fn titles_3<'a>(sites: impl IntoIterator<Item = (&'a str, Duration)>) -> Vec<String> {
    rt::join_all(sites.into_iter().map(|(site, delay)| download(site, delay))).await
}

pub fn e3_1() {
    rt::block_on(async {
        let start = Instant::now();
        println!("Start");
        let titles = titles_3([
            ("rust-lang.org", ms(60)),
            ("docs.rs", ms(30)),
            ("docs2.rs", ms(40)),
        ])
        .await;
        println!("End {0:?} - {1} ms", titles, start.elapsed().as_millis());
    });
}

pub fn e3_2() {
    let start = Instant::now();

    let a = async {
        println!("'a' started");
        slow("a", 30);
        rt::yield_now().await;
        slow("a", 10);
        rt::yield_now().await;
        slow("a", 20);
        rt::yield_now().await;
        println!("'a' finished.");
    };

    let b = async {
        println!("'b' started");
        slow("b", 75);
        rt::yield_now().await;
        slow("b", 10);
        rt::yield_now().await;
        slow("b", 15);
        rt::yield_now().await;
        slow("b", 100);
        println!("'b' finished.");
    };

    rt::block_on(async move {
        println!("{:?}", rt::select(b, a).await);
    });

    println!("duration {}ms", start.elapsed().as_millis());
}

async fn count_slowly(name: &str, n: u32) {
    for i in 1..=n {
        println!("{}: {}", name, i);
        rt::yield_now().await;
    }
}

pub fn e3_3() {
    rt::block_on(async {
        rt::join!(
            count_slowly("A", 3),
            count_slowly("B", 5),
            count_slowly("C", 2)
        );
    });
}

async fn retry<F, Fut, T>(mut f: F, n: u32) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    for _ in 0..n {
        let res = f().await;
        if res.is_some() {
            return res;
        }
        rt::sleep(ms(20)).await;
    }
    None
}

pub fn e3_4() {
    rt::block_on(async {
        let mut c = 0;
        let res = retry(
            || {
                c += 1;
                async move { if c > 3 { Some("done") } else { None } }
            },
            3,
        )
        .await;
        println!("{res:?}");

        c = 0;
        let res = retry(
            || {
                c += 1;
                async move { if c > 3 { Some("done") } else { None } }
            },
            4,
        )
        .await;
        println!("{res:?}");
    });
}

async fn from_select<T>(select: impl Future<Output = rt::Either<T, T>>) -> T {
    match select.await {
        rt::Either::Left(v) | rt::Either::Right(v) => v,
    }
}

type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

async fn first_title<'a>(sites: impl IntoIterator<Item = (&'a str, Duration)>) -> Option<String> {
    let mut iter = sites.into_iter();
    let (site, delay) = iter.next()?;

    let init: BoxFut<'a, String> = Box::pin(download(site, delay));

    let res = iter.fold(init, |acc, (site, delay)| {
        Box::pin(from_select(rt::select(acc, download(site, delay))))
    });

    Some(res.await)
}

pub fn e3_5() {
    rt::block_on(async {
        let res = timeout(
            first_title([
                ("rust-lang.org", ms(60)),
                ("docs.rs", ms(20)),
                ("docs2.rs", ms(40)),
            ]),
            ms(30),
        )
        .await
        .ok()
        .flatten();

        println!("{res:?}")
    });
}
