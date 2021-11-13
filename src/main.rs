pub mod client;
pub mod server;
use std::env;
fn main() {
    let mut args: Vec<_> = env::args().collect();
    assert!(args.len() > 1, "1 command line arg needed: 'client' or 'server'");
    let arg = args.remove(1);
    match arg.as_str() {
        "client" => {
            client::main();
        },
        "server" => {
            server::main();
        },
        _ => panic!("Command line argument needs to be either 'client' or 'server'")
    }
}
