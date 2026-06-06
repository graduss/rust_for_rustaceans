mod c2;

fn main() {
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

    println!("{:?}", c2::into_pieces(String::from("test в test")))
}
