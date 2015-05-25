extern crate docopt;
extern crate rustc_serialize;
extern crate mio;
extern crate roxy;

use std::net::SocketAddr;
use docopt::Docopt;
use roxy::*;

static USAGE: &'static str = "
Usage: 
    roxy-server -s <source> -d <destination>

Options:
    -l  local address
    -r  remote address
";

#[derive(RustcDecodable, Debug)]
struct Args {
    arg_source      : String,
    arg_destination : String,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.decode())
                            .unwrap_or_else(|e| e.exit());

    let src : SocketAddr = args.arg_source.parse().unwrap();
    let dst : SocketAddr = args.arg_destination.parse().unwrap();
    let mut rs = RoxyServer::new(src, dst);
    rs.run();
}
