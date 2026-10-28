//! The Rust Book, глава 17 — основы асинхронного программирования (для начинающих).
//!
//! Теория и задания: `examples/RustBook_Глава_17_Async_практика.md`,
//! первоисточник: <https://doc.rust-lang.org/book/ch17-00-async-await.html>.
//!
//! Книга использует крейт `trpl`; здесь его заменяет учебный `crate::rt`
//! с тем же API. Задержки уменьшены в 10 раз (500 мс -> 50 мс), чтобы
//! `cargo run` не тянулся.

use crate::rt::{self, Either, Stream, StreamExt};
use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ================================================================
// 17.1 Футуры и синтаксис async
// ================================================================

/// Вместо `trpl::get`: сети нет, «страница» скачивается за несколько мс.
struct Response {
    body: String,
}

impl Response {
    async fn text(self) -> String {
        rt::yield_now().await; // тело приходит позже заголовков
        self.body
    }
}

async fn fake_get(url: &str) -> Response {
    println!("    скачиваю {url} ...");
    let (delay, title) = match url {
        "https://www.rust-lang.org" => (80, Some("Rust Programming Language")),
        "https://doc.rust-lang.org/book" => (30, Some("The Rust Programming Language")),
        _ => (50, None),
    };
    rt::sleep(ms(delay)).await;
    let body = match title {
        Some(t) => format!("<html><head><title>{t}</title></head></html>"),
        None => "<html><body>без заголовка</body></html>".to_string(),
    };
    Response { body }
}

/// Вместо `trpl::Html`: вырезаем текст между <title> и </title>.
fn parse_title(html: &str) -> Option<String> {
    let start = html.find("<title>")? + "<title>".len();
    let end = start + html[start..].find("</title>")?;
    Some(html[start..end].to_string())
}

// Листинги 17-1 / 17-2
async fn page_title(url: &str) -> Option<String> {
    let response_text = fake_get(url).await.text().await;
    parse_title(&response_text)
}

/// То же самое без сахара: async fn = обычная fn, возвращающая impl Future.
#[allow(clippy::manual_async_fn)] // именно это и показываем
fn page_title_desugared(url: &str) -> impl Future<Output = Option<String>> {
    async move {
        let text = fake_get(url).await.text().await;
        parse_title(&text)
    }
}

pub fn f1_1() {
    let fut = page_title("https://www.rust-lang.org");
    println!("  футура создана — но ничего ещё не скачивается (футуры ленивы)");
    let title = rt::block_on(fut);
    println!("  заголовок: {title:?}");

    let same = rt::block_on(page_title_desugared("https://www.rust-lang.org"));
    assert_eq!(title, same);
    println!("  async fn и fn -> impl Future дали одно и то же");
}

// Листинг 17-4: async main нельзя (E0752) — входим в async через block_on
pub fn f1_2() {
    let urls = ["https://doc.rust-lang.org/book", "https://example.com"];
    rt::block_on(async {
        for url in urls {
            match page_title(url).await {
                Some(title) => println!("  The title for {url} was {title}"),
                None => println!("  {url} had no title"),
            }
        }
    });
}

async fn page_title_with_url(url: &str) -> (&str, Option<String>) {
    let response_text = fake_get(url).await.text().await;
    (url, parse_title(&response_text))
}

// Листинг 17-5: гонка двух футур
pub fn f1_3() {
    rt::block_on(async {
        let title_fut_1 = page_title_with_url("https://www.rust-lang.org");
        let title_fut_2 = page_title_with_url("https://doc.rust-lang.org/book");

        let (url, maybe_title) = match rt::select(title_fut_1, title_fut_2).await {
            Either::Left(left) => left,
            Either::Right(right) => right,
        };

        println!("  {url} returned first");
        match maybe_title {
            Some(title) => println!("  Its page title was: '{title}'"),
            None => println!("  It had no title."),
        }
    });
}

// ================================================================
// 17.2 Конкурентность с async
// ================================================================

// Листинг 17-6: задача обрывается, когда заканчивается главная футура
pub fn f2_1() {
    rt::block_on(async {
        rt::spawn_task(async {
            for i in 1..10 {
                println!("  hi number {i} from the first task!");
                rt::sleep(ms(50)).await;
            }
        });

        for i in 1..5 {
            println!("  hi number {i} from the second task!");
            rt::sleep(ms(50)).await;
        }
    });
    println!("  (первая задача не досчитала до 9 — её бросили)");
}

// Листинг 17-7: дожидаемся задачи через её JoinHandle
pub fn f2_2() {
    rt::block_on(async {
        let handle = rt::spawn_task(async {
            for i in 1..10 {
                println!("  hi number {i} from the first task!");
                rt::sleep(ms(50)).await;
            }
        });

        for i in 1..5 {
            println!("  hi number {i} from the second task!");
            rt::sleep(ms(50)).await;
        }

        handle.await.unwrap();
    });
}

// Листинг 17-8: join — без задач, две футуры в одной
pub fn f2_3() {
    rt::block_on(async {
        let fut1 = async {
            for i in 1..5 {
                println!("  hi number {i} from the first task!");
                rt::sleep(ms(50)).await;
            }
        };
        let fut2 = async {
            for i in 1..3 {
                println!("  hi number {i} from the second task!");
                rt::sleep(ms(50)).await;
            }
        };
        rt::join(fut1, fut2).await;
    });
}

// Листинг 17-10: внутри одного async-блока всё идёт строго по порядку
pub fn f2_4() {
    rt::block_on(async {
        let start = Instant::now();
        let (tx, mut rx) = rt::channel();

        let vals = vec!["hi", "from", "the", "future"];
        for val in vals {
            tx.send(val).unwrap();
            rt::sleep(ms(50)).await;
        }
        // В книге этой строки нет — и программа там висит вечно (см. 17-12).
        drop(tx);

        while let Some(value) = rx.recv().await {
            println!("  [{:>3} мс] received '{value}'", start.elapsed().as_millis());
        }
    });
}

// Листинги 17-11 + 17-12: отдельные футуры + async move
pub fn f2_5() {
    rt::block_on(async {
        let start = Instant::now();
        let (tx, mut rx) = rt::channel();

        let tx_fut = async move {
            // move: tx принадлежит блоку и дропнется в его конце
            let vals = vec!["hi", "from", "the", "future"];
            for val in vals {
                tx.send(val).unwrap();
                rt::sleep(ms(50)).await;
            }
        };

        let rx_fut = async {
            while let Some(value) = rx.recv().await {
                println!("  [{:>3} мс] received '{value}'", start.elapsed().as_millis());
            }
        };

        rt::join(tx_fut, rx_fut).await;
    });
}

// Листинг 17-13: несколько отправителей и join!
pub fn f2_6() {
    rt::block_on(async {
        let (tx, mut rx) = rt::channel();

        let tx1 = tx.clone();
        let tx1_fut = async move {
            for val in ["hi", "from", "the", "future"] {
                tx1.send(val).unwrap();
                rt::sleep(ms(50)).await;
            }
        };

        let rx_fut = async {
            while let Some(value) = rx.recv().await {
                println!("  received '{value}'");
            }
        };

        let tx_fut = async move {
            for val in ["more", "messages", "for", "you"] {
                tx.send(val).unwrap();
                rt::sleep(ms(150)).await;
            }
        };

        rt::join!(tx1_fut, tx_fut, rx_fut);
    });
}

// ================================================================
// 17.3 Работа с любым числом футур: уступать управление
// ================================================================

/// Листинг 17-14: «медленная» синхронная работа (блокирует поток!)
fn slow(name: &str, ms_: u64) {
    thread::sleep(ms(ms_));
    println!("    '{name}' ran for {ms_}ms");
}

// Листинг 17-15: без точек await футуры не чередуются
pub fn f3_1() {
    rt::block_on(async {
        let a = async {
            println!("    'a' started.");
            slow("a", 30);
            slow("a", 10);
            slow("a", 20);
            rt::sleep(ms(50)).await;
            println!("    'a' finished.");
        };

        let b = async {
            println!("    'b' started.");
            slow("b", 75);
            slow("b", 10);
            slow("b", 15);
            slow("b", 100);
            rt::sleep(ms(50)).await;
            println!("    'b' finished.");
        };

        rt::select(a, b).await;
    });
}

// Листинг 17-17: yield_now — уступаем, ничего не дожидаясь
pub fn f3_2() {
    rt::block_on(async {
        let a = async {
            println!("    'a' started.");
            slow("a", 30);
            rt::yield_now().await;
            slow("a", 10);
            rt::yield_now().await;
            slow("a", 20);
            rt::yield_now().await;
            println!("    'a' finished.");
        };

        let b = async {
            println!("    'b' started.");
            slow("b", 75);
            rt::yield_now().await;
            slow("b", 10);
            rt::yield_now().await;
            slow("b", 15);
            rt::yield_now().await;
            slow("b", 100);
            rt::yield_now().await;
            println!("    'b' finished.");
        };

        rt::select(a, b).await;
    });
}

// Листинг 17-20: своя абстракция — timeout из select + sleep
async fn timeout<F: Future>(future_to_try: F, max_time: Duration) -> Result<F::Output, Duration> {
    match rt::select(future_to_try, rt::sleep(max_time)).await {
        Either::Left(output) => Ok(output),
        Either::Right(_) => Err(max_time),
    }
}

pub fn f3_3() {
    rt::block_on(async {
        for limit in [100, 500] {
            let slow = async {
                rt::sleep(ms(300)).await;
                "Finally finished"
            };
            match timeout(slow, ms(limit)).await {
                Ok(message) => println!("  лимит {limit} мс: Succeeded with '{message}'"),
                Err(d) => println!("  лимит {limit} мс: Failed after {} ms", d.as_millis()),
            }
        }
    });
}

// ================================================================
// 17.4 Потоки (streams): футуры по очереди
// ================================================================

// Листинг 17-22: без `use StreamExt` метода next не будет
pub fn f4_1() {
    rt::block_on(async {
        let values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let iter = values.iter().map(|n| n * 2);
        let mut stream = rt::stream_from_iter(iter);

        while let Some(value) = stream.next().await {
            print!("{value} ");
        }
        println!();
    });
}

// Получатель канала — тоже поток: тот же while let, но значения приходят со временем
pub fn f4_2() {
    rt::block_on(async {
        let start = Instant::now();
        let (tx, mut rx) = rt::channel();
        rt::spawn_task(async move {
            for i in 1..=3 {
                rt::sleep(ms(40)).await;
                tx.send(i * 10).unwrap();
            }
        });
        while let Some(v) = rx.next().await {
            println!("  [{:>3} мс] из потока: {v}", start.elapsed().as_millis());
        }
    });
}

// ================================================================
// 17.5 Трейты async: Future, Pin, Stream
// ================================================================

/// Своя футура: `left` раз отвечает «ещё не готово».
struct Ticks {
    left: u32,
}

impl Future for Ticks {
    type Output = &'static str;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.left == 0 {
            Poll::Ready("готово!")
        } else {
            self.left -= 1;
            cx.waker().wake_by_ref(); // попросить рантайм опросить нас ещё раз
            Poll::Pending
        }
    }
}

pub fn f5_1() {
    // Опрашиваем руками — то, что `.await` делает за нас.
    let mut fut = Ticks { left: 2 };
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match Pin::new(&mut fut).poll(&mut cx) {
            Poll::Pending => println!("  poll -> Pending"),
            Poll::Ready(v) => {
                println!("  poll -> Ready({v:?})");
                break;
            }
        }
    }
    #[allow(clippy::redundant_async_block)] // показываем именно .await
    let via_await = rt::block_on(async { Ticks { left: 2 }.await });
    println!("  а через .await: {via_await}");
}

// Футуры в Vec: нужен pin!
pub fn f5_2() {
    rt::block_on(async {
        let (tx, mut rx) = rt::channel();
        let tx1 = tx.clone();

        let tx1_fut = pin!(async move {
            for val in ["hi", "from", "the", "future"] {
                tx1.send(val).unwrap();
                rt::sleep(ms(50)).await;
            }
        });
        let rx_fut = pin!(async {
            while let Some(value) = rx.recv().await {
                println!("  received '{value}'");
            }
        });
        let tx_fut = pin!(async move {
            for val in ["more", "messages", "for", "you"] {
                tx.send(val).unwrap();
                rt::sleep(ms(150)).await;
            }
        });

        // Разные async-блоки — разные типы, поэтому общий тип — dyn Future.
        // Вариант Vec<Box<dyn Future<Output = ()>>> не компилируется:
        // `dyn Future<Output = ()>` cannot be unpinned.
        let futures: Vec<Pin<&mut dyn Future<Output = ()>>> = vec![tx1_fut, rx_fut, tx_fut];
        rt::join_all(futures).await;
    });
}

/// Свой поток: обратный отсчёт с «задержкой» между значениями.
struct Countdown {
    n: u32,
    waiting: bool,
}

impl Stream for Countdown {
    type Item = u32;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<u32>> {
        if self.n == 0 {
            return Poll::Ready(None); // поток закончился
        }
        if !self.waiting {
            self.waiting = true;
            cx.waker().wake_by_ref();
            return Poll::Pending; // «значение ещё не пришло»
        }
        self.waiting = false;
        self.n -= 1;
        Poll::Ready(Some(self.n + 1))
    }
}

pub fn f5_3() {
    rt::block_on(async {
        let mut stream = Countdown { n: 3, waiting: false };
        while let Some(v) = stream.next().await {
            println!("  {v}...");
        }
        println!("  пуск!");
    });
}

// ================================================================
// 17.6 Футуры, задачи и потоки ОС
// ================================================================

// Листинг 17-25: блокирующая работа в потоке ОС, результаты — через async-канал
pub fn f6_1() {
    let (tx, mut rx) = rt::channel();

    thread::spawn(move || {
        for i in 1..=5 {
            tx.send(i).unwrap();
            thread::sleep(ms(30));
        }
    });

    rt::block_on(async {
        while let Some(message) = rx.recv().await {
            println!("  {message}");
        }
    });
}

// ================================================================
// Задания — решения добавляйте ниже как eX_Y() (см. практику к главе 17).
// ================================================================
