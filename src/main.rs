pub mod server;
use std::env;
fn main() {
    let mut args: Vec<_> = env::args().collect();
    assert!(args.len() > 1, "1 command line arg needed: address to bind to");
    let arg = args.remove(1);
    server::main(&arg)
}
