extern crate rustc_serialize;
extern crate docopt;

use docopt::Docopt;

static USAGE: &'static str = "
Usage: 
    roxy -s <source> -d <destination>

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
    println!("{:?}", args);
}
