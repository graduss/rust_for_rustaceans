//! Глава 9 — асинхронное программирование.
//!
//! Теория и задания: `examples/Глава_9_Асинхронное_программирование_практика.md`,
//! первоисточник: `book/Глава_9_Асинхронное_программирование.md`.
//!
//! Всё написано только на std, без tokio/futures: сами собираем Future,
//! Waker, block_on и мини-исполнитель со spawn, чтобы увидеть, что прячется
//! за async/await.

use std::future::Future;
use std::marker::PhantomPinned;
use std::panic::{self, AssertUnwindSafe};
use std::pin::{Pin, pin};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ================================================================
// 0. Инфраструктура: block_on — исполнитель для одной футуры
// ================================================================

/// Waker, который будит поток исполнителя (thread::unpark) и считает вызовы.
struct ThreadWaker {
    thread: thread::Thread,
    wakes: AtomicUsize,
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
        self.thread.unpark();
    }
}

fn thread_waker() -> (Arc<ThreadWaker>, Waker) {
    let tw = Arc::new(ThreadWaker {
        thread: thread::current(),
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(tw.clone());
    (tw, waker)
}

/// Опрашивает футуру; пока она Pending — спит, пока её не разбудят.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    block_on_stats(fut).0
}

/// block_on + статистика: (результат, сколько раз опросили, сколько раз будили).
fn block_on_stats<F: Future>(fut: F) -> (F::Output, usize, usize) {
    let mut fut = pin!(fut); // закрепление на стеке (раздел 3)
    let (tw, waker) = thread_waker();
    let mut cx = Context::from_waker(&waker);
    let mut polls = 0;
    loop {
        polls += 1;
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return (v, polls, tw.wakes.load(Ordering::Relaxed));
        }
        thread::park(); // засыпаем до wake (раздел 4)
    }
}

/// Как block_on, но сдаётся, если за `timeout` футуру так и не разбудили.
fn block_on_timeout<F: Future>(fut: F, timeout: Duration) -> Option<F::Output> {
    let mut fut = pin!(fut);
    let (_tw, waker) = thread_waker();
    let mut cx = Context::from_waker(&waker);
    let deadline = Instant::now() + timeout;
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return Some(v);
        }
        thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
        if Instant::now() >= deadline {
            return None;
        }
    }
}

// ================================================================
// 1. Poll и трейт Future
// ================================================================

/// Футура, которая `left` раз отвечает Pending, а затем разрешается в `value`.
pub struct Countdown {
    left: u32,
    value: u32,
}

impl Countdown {
    pub fn new(left: u32, value: u32) -> Self {
        Countdown { left, value }
    }
}

impl Future for Countdown {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        if self.left == 0 {
            return Poll::Ready(self.value);
        }
        self.left -= 1;
        // Контракт poll: вернули Pending => обязаны устроить wake.
        // «Событие» уже наступило, поэтому будим сразу.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

pub fn f1_1() {
    let mut cd = Countdown::new(3, 42);
    // Waker, который ничего не делает: опрашиваем вручную в цикле.
    let mut cx = Context::from_waker(Waker::noop());
    let mut i = 0;
    loop {
        i += 1;
        // Countdown: Unpin => Pin::new безопасен (раздел 3).
        match Pin::new(&mut cd).poll(&mut cx) {
            Poll::Pending => println!("poll #{i}: Pending"),
            Poll::Ready(v) => {
                println!("poll #{i}: Ready({v})");
                break;
            }
        }
    }
}

async fn say(msg: &str) -> usize {
    println!("  выполняется say({msg:?})");
    msg.len()
}

pub fn f1_2() {
    let fut = say("ленивая");
    println!("футура создана — say ещё ничего не напечатала");
    let n = block_on(fut);
    println!("block_on вернул {n}");

    let (v, polls, wakes) = block_on_stats(Countdown::new(3, 7));
    println!("Countdown(3): {v}, опросов {polls}, пробуждений {wakes}");
}

pub fn f1_3() {
    let mut fut = pin!(async { 1 });
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready(1));

    // async-футура не «слитая» (fused): повторный poll после Ready — паника.
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let again = panic::catch_unwind(AssertUnwindSafe(|| fut.as_mut().poll(&mut cx)));
    panic::set_hook(prev_hook);
    println!("poll после Ready запаниковал: {}", again.is_err());
}

// ================================================================
// 2. Машины состояний, async/await и генераторы
// ================================================================

/// Уступить исполнителю один раз: Pending + немедленный wake.
pub struct YieldNow(bool);

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

async fn two_steps() -> u32 {
    let a = Countdown::new(2, 10).await;
    let b = Countdown::new(1, 20).await;
    a + b
}

/// Ручная машина состояний, эквивалентная `two_steps` (ср. листинг 9-4).
enum TwoSteps {
    First(Countdown),
    Second { a: u32, fut: Countdown },
    Done,
}

impl Future for TwoSteps {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        // loop — аналог `return self.poll()` из листинга 9-4: продвинулись —
        // сразу пробуем следующий шаг.
        loop {
            match &mut *self {
                TwoSteps::First(fut) => match Pin::new(fut).poll(cx) {
                    Poll::Ready(a) => {
                        *self = TwoSteps::Second {
                            a,
                            fut: Countdown::new(1, 20),
                        }
                    }
                    Poll::Pending => return Poll::Pending,
                },
                TwoSteps::Second { a, fut } => match Pin::new(fut).poll(cx) {
                    Poll::Ready(b) => {
                        let sum = *a + b;
                        *self = TwoSteps::Done;
                        return Poll::Ready(sum);
                    }
                    Poll::Pending => return Poll::Pending,
                },
                TwoSteps::Done => panic!("TwoSteps опрошена после Ready"),
            }
        }
    }
}

pub fn f2_1() {
    let (v1, p1, _) = block_on_stats(two_steps());
    let (v2, p2, _) = block_on_stats(TwoSteps::First(Countdown::new(2, 10)));
    println!("async fn:       {v1}, опросов {p1}");
    println!("ручная машина:  {v2}, опросов {p2}");
}

async fn big_across_await() -> u8 {
    let buf = [1u8; 8192];
    yield_now().await; // buf жив через точку await => хранится в футуре
    buf[0]
}

async fn big_before_await() -> u8 {
    let x = {
        let buf = [1u8; 8192];
        buf[0]
    }; // buf умер до await => в футуру не попадает
    yield_now().await;
    x
}

async fn nested() -> u8 {
    big_across_await().await + 1 // внутренняя футура хранится целиком
}

async fn nested_boxed() -> u8 {
    Box::pin(big_across_await()).await + 1 // хранится только указатель
}

pub fn f2_2() {
    use std::mem::size_of_val;
    println!("Countdown:              {:>5} байт", size_of_val(&Countdown::new(1, 1)));
    println!("two_steps():            {:>5} байт", size_of_val(&two_steps()));
    println!("big_across_await():     {:>5} байт", size_of_val(&big_across_await()));
    println!("big_before_await():     {:>5} байт", size_of_val(&big_before_await()));
    println!("nested():               {:>5} байт", size_of_val(&nested()));
    println!("nested_boxed():         {:>5} байт", size_of_val(&nested_boxed()));
    assert_eq!(block_on(nested()), block_on(nested_boxed()));
}

pub fn f2_3() {
    let fut = async {
        let arr = [1, 2, 3];
        let first = &arr[0]; // ссылка на локальную переменную самой футуры...
        yield_now().await; // ...переживает точку await => футура самоссылочная
        *first + arr.len() as i32
    };
    // Поэтому она !Unpin: `Pin::new(&mut fut)` не скомпилируется (раздел 3).
    println!("ссылка через await: {}", block_on(fut));
}

// ================================================================
// 3. Pin и Unpin
// ================================================================

/// Наивная самоссылочная структура: ptr указывает на собственное поле data.
struct Naive {
    data: String,
    ptr: *const String,
}

/// То же, но закреплённое: PhantomPinned делает тип !Unpin.
struct Pinned {
    data: String,
    ptr: *const String,
    _pin: PhantomPinned,
}

impl Pinned {
    fn new(data: &str) -> Pin<Box<Self>> {
        let mut boxed = Box::pin(Pinned {
            data: data.into(),
            ptr: std::ptr::null(),
            _pin: PhantomPinned,
        });
        let ptr: *const String = &boxed.data;
        // SAFETY: только записываем поле, значение из Pin не перемещаем.
        unsafe { boxed.as_mut().get_unchecked_mut().ptr = ptr };
        boxed
    }

    fn via_ptr(self: Pin<&Self>) -> &str {
        // SAFETY: self закреплён => data не переместилась, ptr валиден.
        unsafe { &*self.ptr }
    }
}

pub fn f3_1() {
    let mut a = Naive {
        data: "naive".into(),
        ptr: std::ptr::null(),
    };
    a.ptr = &a.data;
    println!("Naive до перемещения:   &data = {:p}, ptr = {:p}", &a.data, a.ptr);
    let b = Box::new(a); // перемещение = memcpy, ptr никто не обновил
    println!("Naive после:            &data = {:p}, ptr = {:p}  <- висячий", &b.data, b.ptr);

    let p = Pinned::new("pinned");
    println!("Pinned:                 &data = {:p}, ptr = {:p}", &p.data, p.ptr);
    let moved = p; // двигаем Pin<Box<_>> — т.е. указатель, сама структура на месте
    println!("Pinned после move:      &data = {:p}, ptr = {:p}", &moved.data, moved.ptr);
    println!("через ptr: {:?}", moved.as_ref().via_ptr());
    // let mut other = Pinned::new("x");
    // std::mem::swap(&mut *moved, &mut *other); // ОШИБКА: Pinned: !Unpin => нет DerefMut
}

fn assert_unpin<T: Unpin>(_: &T) {}

pub fn f3_2() {
    let mut cd = Countdown::new(1, 5);
    assert_unpin(&cd); // все поля Unpin => авто-трейт выведен
    let _pinned = Pin::new(&mut cd); // безопасный конструктор только для Unpin

    let fut = async {
        yield_now().await;
        1
    };
    // assert_unpin(&fut);        // ОШИБКА: `PhantomPinned`/async block cannot be unpinned
    // let _ = Pin::new(&mut fut); // та же ошибка

    let boxed = Box::pin(fut); // закрепление в куче, без unsafe
    assert_unpin(&boxed); // Pin<Box<T>>: Unpin всегда — двигается указатель, не T
    let on_stack = pin!(async { 2 }); // закрепление на стеке, тип Pin<&mut {async}>
    assert_unpin(&on_stack);

    println!("Box::pin -> {}, pin! -> {}", block_on(boxed), block_on(on_stack));
}

/// Закрепление на стеке из листинга 9-11 (в std теперь есть `pin!`).
macro_rules! pin_mut {
    ($var:ident) => {
        // Перемещаем значение в новую привязку: старое имя больше не доступно.
        let mut $var = $var;
        // SAFETY: $var затенён — до незакреплённого значения не добраться.
        let mut $var = unsafe { Pin::new_unchecked(&mut $var) };
    };
}

pub fn f3_3() {
    let fut = async {
        yield_now().await;
        yield_now().await;
        3
    };
    pin_mut!(fut); // fut: Pin<&mut {async block}>
    let mut cx = Context::from_waker(Waker::noop());
    let mut polls = 1;
    while fut.as_mut().poll(&mut cx).is_pending() {
        polls += 1;
    }
    println!("pin_mut!: готово за {polls} опроса");

    // Pin::set: безопасно заменить закреплённое значение — старое дропается на месте.
    let mut p = Box::pin(two_steps());
    let _ = p.as_mut().poll(&mut cx); // начали выполнять
    p.set(two_steps()); // тот же тип: та же async fn
    println!("после Pin::set заново: {}", block_on(p));
}

// ================================================================
// 4. Засыпание: Waker и контракт poll
// ================================================================

/// Листовая футура «таймер». Внешнее событие эмулирует отдельный поток.
pub struct Delay {
    dur: Duration,
    started: bool,
    shared: Arc<Mutex<DelayState>>,
}

struct DelayState {
    done: bool,
    waker: Option<Waker>,
}

impl Delay {
    pub fn new(dur: Duration) -> Self {
        Delay {
            dur,
            started: false,
            shared: Arc::new(Mutex::new(DelayState {
                done: false,
                waker: None,
            })),
        }
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        {
            let mut st = self.shared.lock().unwrap();
            if st.done {
                return Poll::Ready(());
            }
            // Сохраняем актуальный Waker: между опросами футуру могли
            // переместить в другую задачу, и старый Waker уже не тот.
            match &st.waker {
                Some(w) if w.will_wake(cx.waker()) => {}
                _ => st.waker = Some(cx.waker().clone()),
            }
        }
        if !self.started {
            // Футуры ленивы: таймер стартует при первом опросе.
            self.started = true;
            let (shared, dur) = (self.shared.clone(), self.dur);
            thread::spawn(move || {
                thread::sleep(dur);
                let mut st = shared.lock().unwrap();
                st.done = true;
                if let Some(w) = st.waker.take() {
                    w.wake();
                }
            });
        }
        Poll::Pending
    }
}

pub fn f4_1() {
    let t = Instant::now();
    let ((), polls, wakes) = block_on_stats(Delay::new(ms(100)));
    println!(
        "Delay(100 мс): {} мс, опросов {polls}, пробуждений {wakes}",
        t.elapsed().as_millis()
    );
}

/// Waker, собранный вручную из vtable (ср. RawWakerVTable в книге).
fn logging_waker() -> Waker {
    fn clone(p: *const ()) -> RawWaker {
        println!("    [vtable] clone");
        RawWaker::new(p, &VTABLE)
    }
    fn wake(_: *const ()) {
        println!("    [vtable] wake");
    }
    fn wake_by_ref(_: *const ()) {
        println!("    [vtable] wake_by_ref");
    }
    fn drop_waker(_: *const ()) {
        println!("    [vtable] drop");
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_waker);
    // SAFETY: data не используется; функции vtable соблюдают контракт RawWaker.
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

pub fn f4_2() {
    let waker = logging_waker();
    let mut cx = Context::from_waker(&waker);

    println!("  опрос Countdown(1):");
    let mut cd = Countdown::new(1, 0);
    let _ = Pin::new(&mut cd).poll(&mut cx);

    println!("  опрос Delay(50 мс):");
    let mut delay = Delay::new(ms(50));
    let _ = Pin::new(&mut delay).poll(&mut cx);
    thread::sleep(ms(100)); // «исполнитель» ждёт; wake приходит из потока таймера
    println!("  повторный опрос Delay: {:?}", Pin::new(&mut delay).poll(&mut cx));

    println!("  конец функции:");
}

/// НЕВЕРНО: возвращает Pending, но никому не отдаёт Waker.
struct Forgetful {
    polled: bool,
}

impl Future for Forgetful {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.polled {
            return Poll::Ready(());
        }
        self.polled = true;
        Poll::Pending // готова уже к следующему опросу, но никто об этом не узнает
    }
}

pub fn f4_3() {
    let r = block_on_timeout(Forgetful { polled: false }, ms(200));
    println!("Forgetful без wake:  {r:?}  <- без таймаута висели бы вечно");
    let r = block_on_timeout(Countdown::new(1, 1), ms(200));
    println!("Countdown c wake:    {r:?}");
}

/// Субисполнитель: ждёт обе футуры (упрощённый join).
/// Опрашивает обе на каждом poll — настоящий join оборачивает Waker,
/// чтобы знать, кого именно разбудили (задание 4.3).
pub struct Join2<A: Future, B: Future> {
    a: Pin<Box<A>>,
    b: Pin<Box<B>>,
    ra: Option<A::Output>,
    rb: Option<B::Output>,
}

// Футуры уже лежат в Pin<Box<_>>, а к результатам мы никогда не выдаём
// Pin<&mut _> => Join2 можно безопасно объявить Unpin (и пользоваться get_mut).
impl<A: Future, B: Future> Unpin for Join2<A, B> {}

pub fn join2<A: Future, B: Future>(a: A, b: B) -> Join2<A, B> {
    Join2 {
        a: Box::pin(a),
        b: Box::pin(b),
        ra: None,
        rb: None,
    }
}

impl<A: Future, B: Future> Future for Join2<A, B> {
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

async fn worker(name: &'static str, steps: u32) -> u32 {
    for i in 1..=steps {
        println!("    {name}: шаг {i}");
        yield_now().await;
    }
    steps
}

pub fn f4_4() {
    let (a, b) = block_on(join2(worker("A", 3), worker("B", 2)));
    println!("  join2 -> ({a}, {b}) — шаги чередуются в одном потоке");

    let t = Instant::now();
    block_on(join2(Delay::new(ms(100)), Delay::new(ms(100))));
    println!("  два Delay(100 мс) через join2: {} мс", t.elapsed().as_millis());
}

// ================================================================
// 5. spawn: мини-исполнитель с задачами
// ================================================================

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Задача — корневая футура + способ поставить себя в очередь при wake.
struct Task {
    fut: Mutex<Option<BoxFuture>>,
    queue: mpsc::Sender<Arc<Task>>,
}

impl Wake for Task {
    // wake = «задача готова к выполнению»: кладём её в очередь исполнителя.
    fn wake(self: Arc<Self>) {
        let queue = self.queue.clone();
        let _ = queue.send(self);
    }
}

#[derive(Clone)]
pub struct Spawner {
    queue: mpsc::Sender<Arc<Task>>,
    live: Arc<AtomicUsize>,
}

impl Spawner {
    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        self.live.fetch_add(1, Ordering::SeqCst);
        let task = Arc::new(Task {
            fut: Mutex::new(Some(Box::pin(fut))),
            queue: self.queue.clone(),
        });
        self.queue.send(task).unwrap();
    }
}

/// Однопоточный исполнитель: очередь готовых задач, спим на recv().
pub struct Executor {
    ready: mpsc::Receiver<Arc<Task>>,
    spawner: Spawner,
}

impl Executor {
    pub fn new() -> Self {
        let (queue, ready) = mpsc::channel();
        Executor {
            ready,
            spawner: Spawner {
                queue,
                live: Arc::new(AtomicUsize::new(0)),
            },
        }
    }

    pub fn spawner(&self) -> Spawner {
        self.spawner.clone()
    }

    /// Опрашивает готовые задачи, пока не завершатся все порождённые.
    pub fn run(&self) {
        while self.spawner.live.load(Ordering::SeqCst) > 0 {
            // Нет готовых задач => спим, пока кто-нибудь не вызовет wake.
            let task = self.ready.recv().unwrap();
            let Some(mut fut) = task.fut.lock().unwrap().take() else {
                continue; // лишнее пробуждение уже завершённой задачи
            };
            // Свой Waker на каждую задачу — так wake знает, кого будить.
            let waker = Waker::from(task.clone());
            let mut cx = Context::from_waker(&waker);
            if fut.as_mut().poll(&mut cx).is_pending() {
                *task.fut.lock().unwrap() = Some(fut);
            } else {
                self.spawner.live.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }
}

async fn handle_client(id: u32, start: Instant) {
    println!("    [{:>3} мс] клиент {id}: начал", start.elapsed().as_millis());
    Delay::new(ms(100)).await; // «читаем запрос»
    Delay::new(ms(100)).await; // «пишем ответ»
    println!("    [{:>3} мс] клиент {id}: закончил", start.elapsed().as_millis());
}

pub fn f5_1() {
    println!("  листинг 9-14: await по очереди");
    let start = Instant::now();
    block_on(async {
        for id in 1..=3 {
            handle_client(id, start).await;
        }
    });
    println!("  итого: {} мс", start.elapsed().as_millis());

    println!("  листинг 9-16: spawn на каждого клиента");
    let ex = Executor::new();
    let spawner = ex.spawner();
    let start = Instant::now();
    let inner = spawner.clone();
    spawner.spawn(async move {
        // «сервер»: принимает соединения и порождает задачу на каждое
        for id in 1..=3 {
            inner.spawn(handle_client(id, start));
        }
    });
    ex.run();
    println!("  итого: {} мс", start.elapsed().as_millis());
}

fn ticker_with_neighbor(blocking: bool) {
    let ex = Executor::new();
    let spawner = ex.spawner();
    let start = Instant::now();
    spawner.spawn(async move {
        for i in 1..=3 {
            println!("    [{:>3} мс] тикер: тик {i}", start.elapsed().as_millis());
            Delay::new(ms(50)).await;
        }
    });
    spawner.spawn(async move {
        if blocking {
            thread::sleep(ms(300)); // блокирует поток исполнителя целиком
        } else {
            Delay::new(ms(300)).await; // уступает исполнителю
        }
        println!("    [{:>3} мс] сосед: проснулся", start.elapsed().as_millis());
    });
    ex.run();
}

pub fn f5_2() {
    println!("  сосед делает thread::sleep(300 мс):");
    ticker_with_neighbor(true);
    println!("  сосед делает Delay(300 мс).await:");
    ticker_with_neighbor(false);
}

// ================================================================
// Задания — решения добавляйте ниже как eX_Y() (см. практику главы 9).
// ================================================================
