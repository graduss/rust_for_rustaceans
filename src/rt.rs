//! Учебный мини-рантайм на std с API как у крейта `trpl` из The Rust Book (гл. 17).
//!
//! Нужен, чтобы примеры книги запускались без зависимостей: где в книге
//! `trpl::block_on`, у нас `rt::block_on`, и так далее. Однопоточный и
//! намеренно простой; как устроены такие рантаймы изнутри — глава 9
//! (`src/c9.rs`).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::{Pin, pin};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

// ================================================================
// block_on и spawn_task
// ================================================================

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Состояние рантайма: очередь готовых задач и флаг «главную футуру разбудили».
struct Shared {
    ready: Mutex<VecDeque<Arc<Task>>>,
    main_woken: AtomicBool,
    closed: AtomicBool,
    thread: thread::Thread,
}

struct Task {
    fut: Mutex<Option<BoxFuture>>,
    shared: Arc<Shared>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        let shared = self.shared.clone();
        if shared.closed.load(Ordering::SeqCst) {
            return; // block_on уже вернулся — задачу никто не опросит
        }
        shared.ready.lock().unwrap().push_back(self);
        shared.thread.unpark();
    }
}

struct MainWaker(Arc<Shared>);

impl Wake for MainWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.main_woken.store(true, Ordering::SeqCst);
        self.0.thread.unpark();
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

/// Выполняет футуру до конца, блокируя текущий поток (аналог `trpl::block_on`).
/// Пока она ждёт, опрашивает задачи из `spawn_task`. Когда главная футура
/// завершилась, незаконченные задачи бросаются.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let shared = Arc::new(Shared {
        ready: Mutex::new(VecDeque::new()),
        main_woken: AtomicBool::new(true),
        closed: AtomicBool::new(false),
        thread: thread::current(),
    });
    let prev = CURRENT.replace(Some(shared.clone()));
    let waker = Waker::from(Arc::new(MainWaker(shared.clone())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);

    let out = loop {
        if shared.main_woken.swap(false, Ordering::SeqCst)
            && let Poll::Ready(v) = fut.as_mut().poll(&mut cx)
        {
            break v;
        }
        // Опрашиваем только задачи, готовые к этому моменту, — чтобы задача,
        // которая постоянно уступает, не заморила главную футуру.
        let n = shared.ready.lock().unwrap().len();
        for _ in 0..n {
            let Some(task) = shared.ready.lock().unwrap().pop_front() else {
                break;
            };
            let Some(mut task_fut) = task.fut.lock().unwrap().take() else {
                continue; // задача уже завершилась
            };
            let task_waker = Waker::from(task.clone());
            if task_fut
                .as_mut()
                .poll(&mut Context::from_waker(&task_waker))
                .is_pending()
            {
                *task.fut.lock().unwrap() = Some(task_fut);
            }
        }
        if !shared.main_woken.load(Ordering::SeqCst) && shared.ready.lock().unwrap().is_empty() {
            thread::park(); // спим, пока кто-нибудь не вызовет wake
        }
    };

    shared.closed.store(true, Ordering::SeqCst);
    shared.ready.lock().unwrap().clear();
    CURRENT.set(prev);
    out
}

struct JoinState<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

/// Футура-«ручка» задачи: разрешается результатом задачи.
pub struct JoinHandle<T> {
    state: Arc<Mutex<JoinState<T>>>,
}

impl<T> Future for JoinHandle<T> {
    // В tokio здесь JoinError (задача запаниковала или отменена);
    // у нас ошибок не бывает, но `.unwrap()` из книги работает так же.
    type Output = Result<T, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.state.lock().unwrap();
        match st.value.take() {
            Some(v) => Poll::Ready(Ok(v)),
            None => {
                st.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Запускает футуру отдельной задачей (аналог `trpl::spawn_task`).
/// Работает только внутри `block_on`.
pub fn spawn_task<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let shared = CURRENT
        .with_borrow(|c| c.clone())
        .expect("spawn_task можно вызывать только внутри block_on");
    let state = Arc::new(Mutex::new(JoinState {
        value: None,
        waker: None,
    }));
    let task_state = state.clone();
    let task = Arc::new(Task {
        fut: Mutex::new(Some(Box::pin(async move {
            let v = fut.await;
            let mut st = task_state.lock().unwrap();
            st.value = Some(v);
            if let Some(w) = st.waker.take() {
                w.wake();
            }
        }))),
        shared: shared.clone(),
    });
    shared.ready.lock().unwrap().push_back(task);
    JoinHandle { state }
}

// ================================================================
// sleep и yield_now
// ================================================================

/// Футура из `sleep`. Таймер — отдельный поток (в tokio — общий реактор).
pub struct Sleep {
    dur: Duration,
    started: bool,
    state: Arc<Mutex<(bool, Option<Waker>)>>,
}

/// Асинхронный сон: в отличие от `thread::sleep`, уступает рантайму.
pub fn sleep(dur: Duration) -> Sleep {
    Sleep {
        dur,
        started: false,
        state: Arc::new(Mutex::new((false, None))),
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        {
            let mut st = self.state.lock().unwrap();
            if st.0 {
                return Poll::Ready(());
            }
            st.1 = Some(cx.waker().clone());
        }
        if !self.started {
            self.started = true;
            let (state, dur) = (self.state.clone(), self.dur);
            thread::spawn(move || {
                thread::sleep(dur);
                let mut st = state.lock().unwrap();
                st.0 = true;
                if let Some(w) = st.1.take() {
                    w.wake();
                }
            });
        }
        Poll::Pending
    }
}

pub struct YieldNow(bool);

/// Отдать управление рантайму, ничего не дожидаясь.
pub fn yield_now() -> YieldNow {
    YieldNow(false)
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

// ================================================================
// join, join!, join_all, select
// ================================================================

/// Футура из `join`.
pub struct Join<A: Future, B: Future> {
    a: Pin<Box<A>>,
    b: Pin<Box<B>>,
    ra: Option<A::Output>,
    rb: Option<B::Output>,
}

// Футуры лежат в Pin<Box<_>>, результаты не закрепляются => можно Unpin.
impl<A: Future, B: Future> Unpin for Join<A, B> {}

/// Ждёт обе футуры; опрашивает их по очереди (честно), сначала `a`.
pub fn join<A: Future, B: Future>(a: A, b: B) -> Join<A, B> {
    Join {
        a: Box::pin(a),
        b: Box::pin(b),
        ra: None,
        rb: None,
    }
}

impl<A: Future, B: Future> Future for Join<A, B> {
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.ra.is_none()
            && let Poll::Ready(v) = this.a.as_mut().poll(cx)
        {
            this.ra = Some(v);
        }
        if this.rb.is_none()
            && let Poll::Ready(v) = this.b.as_mut().poll(cx)
        {
            this.rb = Some(v);
        }
        match (this.ra.take(), this.rb.take()) {
            (Some(a), Some(b)) => Poll::Ready((a, b)),
            (ra, rb) => {
                (this.ra, this.rb) = (ra, rb);
                Poll::Pending
            }
        }
    }
}

/// `join!(a, b, c, ...)` — дождаться фиксированного числа футур.
/// Как и в книге, сам делает `.await`, поэтому пишется только внутри async.
macro_rules! join_macro {
    ($a:expr $(,)?) => {
        ($a).await
    };
    ($a:expr, $($rest:expr),+ $(,)?) => {
        $crate::rt::join($a, async move { $crate::rt::join!($($rest),+) }).await
    };
}
pub(crate) use join_macro as join; // макросы и функции — разные пространства имён

/// Футура из `join_all`.
pub struct JoinAll<F: Future> {
    futs: Vec<Pin<Box<F>>>,
    results: Vec<Option<F::Output>>,
}

impl<F: Future> Unpin for JoinAll<F> {}

/// Ждёт все футуры из коллекции (их число известно только во время выполнения).
pub fn join_all<F: Future>(futs: impl IntoIterator<Item = F>) -> JoinAll<F> {
    let futs: Vec<_> = futs.into_iter().map(Box::pin).collect();
    let results = futs.iter().map(|_| None).collect();
    JoinAll { futs, results }
}

impl<F: Future> Future for JoinAll<F> {
    type Output = Vec<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        for (fut, slot) in this.futs.iter_mut().zip(this.results.iter_mut()) {
            if slot.is_none()
                && let Poll::Ready(v) = fut.as_mut().poll(cx)
            {
                *slot = Some(v);
            }
        }
        if this.results.iter().all(Option::is_some) {
            Poll::Ready(this.results.iter_mut().map(|r| r.take().unwrap()).collect())
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

/// Футура из `select`.
pub struct Select<A, B> {
    a: Pin<Box<A>>,
    b: Pin<Box<B>>,
}

/// «Гонка»: результат той футуры, что закончит первой. Опрашивает по порядку
/// аргументов, поэтому при одновременной готовности побеждает `a`.
/// Проигравшая футура просто дропается.
pub fn select<A: Future, B: Future>(a: A, b: B) -> Select<A, B> {
    Select {
        a: Box::pin(a),
        b: Box::pin(b),
    }
}

impl<A: Future, B: Future> Future for Select<A, B> {
    type Output = Either<A::Output, B::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(v) = self.a.as_mut().poll(cx) {
            return Poll::Ready(Either::Left(v));
        }
        if let Poll::Ready(v) = self.b.as_mut().poll(cx) {
            return Poll::Ready(Either::Right(v));
        }
        Poll::Pending
    }
}

// ================================================================
// channel — асинхронный неограниченный mpsc-канал
// ================================================================

struct Chan<T> {
    queue: VecDeque<T>,
    senders: usize,
    receiver_alive: bool,
    rx_waker: Option<Waker>,
}

pub struct Sender<T>(Arc<Mutex<Chan<T>>>);
pub struct Receiver<T>(Arc<Mutex<Chan<T>>>);

/// Получатель уже дропнут; внутри — неотправленное значение.
pub struct SendError<T>(pub T);

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SendError(..)")
    }
}

/// Асинхронный канал (аналог `trpl::channel`). Канал закрывается,
/// когда дропнуты ВСЕ отправители — тогда `recv` вернёт `None`.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let chan = Arc::new(Mutex::new(Chan {
        queue: VecDeque::new(),
        senders: 1,
        receiver_alive: true,
        rx_waker: None,
    }));
    (Sender(chan.clone()), Receiver(chan))
}

impl<T> Sender<T> {
    /// Не async: канал неограниченный, ждать места не нужно.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        let mut chan = self.0.lock().unwrap();
        if !chan.receiver_alive {
            return Err(SendError(value));
        }
        chan.queue.push_back(value);
        if let Some(w) = chan.rx_waker.take() {
            w.wake();
        }
        Ok(())
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.0.lock().unwrap().senders += 1;
        Sender(self.0.clone())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut chan = self.0.lock().unwrap();
        chan.senders -= 1;
        if chan.senders == 0
            && let Some(w) = chan.rx_waker.take()
        {
            w.wake(); // последний отправитель ушёл — пусть recv вернёт None
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.0.lock().unwrap().receiver_alive = false;
    }
}

impl<T> Receiver<T> {
    pub fn recv(&mut self) -> Recv<'_, T> {
        Recv(self)
    }

    fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let mut chan = self.0.lock().unwrap();
        if let Some(v) = chan.queue.pop_front() {
            return Poll::Ready(Some(v));
        }
        if chan.senders == 0 {
            return Poll::Ready(None);
        }
        chan.rx_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

/// Футура из `Receiver::recv`.
pub struct Recv<'a, T>(&'a mut Receiver<T>);

impl<T> Future for Recv<'_, T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.0.poll_recv(cx)
    }
}

// ================================================================
// Stream и StreamExt
// ================================================================

/// Асинхронный итератор: `Iterator` + `Future` (в std его пока нет).
pub trait Stream {
    type Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
}

/// Удобные методы поверх `Stream` — реализованы для всех потоков сразу.
pub trait StreamExt: Stream {
    fn next(&mut self) -> Next<'_, Self>
    where
        Self: Unpin,
    {
        Next(self)
    }
}

impl<S: Stream + ?Sized> StreamExt for S {}

/// Футура из `StreamExt::next`.
pub struct Next<'a, S: ?Sized>(&'a mut S);

impl<S: Stream + Unpin + ?Sized> Future for Next<'_, S> {
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.0).poll_next(cx)
    }
}

/// Поток из `stream_from_iter`.
pub struct Iter<I>(I);

impl<I> Unpin for Iter<I> {}

/// Превращает любой итератор в поток (аналог `trpl::stream_from_iter`).
pub fn stream_from_iter<I: IntoIterator>(iter: I) -> Iter<I::IntoIter> {
    Iter(iter.into_iter())
}

impl<I: Iterator> Stream for Iter<I> {
    type Item = I::Item;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<I::Item>> {
        Poll::Ready(self.get_mut().0.next())
    }
}

/// Получатель канала — тоже поток: `while let Some(v) = rx.next().await`.
impl<T> Stream for Receiver<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.get_mut().poll_recv(cx)
    }
}
