mod c2;

fn test_c2() {
    c2::f1_1();
    c2::f1_2();
    c2::f3_1();

    c2::e1_1();
    c2::e2_2();

    println!("{:?}", c2::get_config());

    let conf: c2::TConfig = [1, 2, 3, 4];
    unsafe {
        c2::e3_init(&conf);
    }
    println!("{:?} - {:?}", conf, c2::get_config());
    unsafe {
        c2::e3_init(&[22, 33, 44, 55]);
    }
    println!("{:?} - {:?}", conf, c2::get_config());

    println!("{:?}", c2::into_pieces(String::from("test в test")));

    let v: &'static mut Vec<i32> = c2::e3_2();
    v.push(1);
    v.push(2);

    (0..2)
        .into_iter()
        .map(|_| {
            let v: &'static Vec<i32> = v;
            std::thread::spawn(move || println!("{:?}", v))
        })
        .for_each(|job| job.join().unwrap());

    println!("{:?} \n", v);

    println!("## E 6.2");
    let fib = c2::Fib::new(20);
    println!("Fib[15] = {}", fib.get(15));
    println!("======== \n");

    println!("## E 6.3");
    let counter = c2::Counter::new();
    println!("0 = {}", counter.get());
    counter.record();
    counter.record();
    println!("{}", counter.get());
    println!("===== \n");

    println!("### P 7");
    let doc = String::from("test dock last");
    let remaining: &str;
    {
        let mut p = c2::Parser::new(&doc);
        println!("{:?}", p.next_world());
        println!("{:?}", p.next_world());
        remaining = p.remaining();
    }
    println!("{remaining}");
    println!("==== \n");

    println!("### E 7.4");
    let doc = String::from("test, dock, last");
    let first: &str;

    {
        let decim = String::from(", ");
        let mut splet = c2::StrSplit::new(&doc, &decim);
        first = splet.next().unwrap();

        std::thread::scope(|s| {
            for chank in splet {
                s.spawn(move || {
                    println!("{chank}");
                });
            }
        })
    }

    println!("{first}");
    println!("==== \n");

    println!("### e 8.3");
    let string = String::from("test");
    let mut local: &str = &string;
    *c2::MutStr { s: &mut local }.s = "test 2";
    println!("{local}");
    println!("==== \n");

    println!("### project");
    let mut area = c2::Arena::new();
    println!("{}", area.log(String::from("test 1")));
    println!("{}", area.log(String::from("test 2")));
    let _ = area.log(String::from("test 3"));
    println!("count: {}", area.count());
    area.drain()
        .into_iter()
        .for_each(|string| println!("{:?}", string));

    let mut area2 = c2::Arena::new();
    let demo_log = {
        let s = "test 2";
        area2.log(s.into())
    };
    println!("demo_log: {demo_log}");
    println!("==== \n");
}

fn main() {
    test_c2();
}
