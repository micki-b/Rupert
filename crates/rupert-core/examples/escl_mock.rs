//! Dev tool: a simulated eSCL scanner on this computer only (127.0.0.1;
//! not announced on the network) that "scans" the given JPEG pages. For
//! trying the scan screen without hardware: add it by address
//! `127.0.0.1:<port>`. Runs until stopped.
//! Usage: cargo run -p rupert-core --example escl_mock -- <port> <page.jpg>...

use rupert_core::escl::mock::MockScanner;

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().expect("port").parse().expect("port number");
    let pages: Vec<Vec<u8>> = args.map(|p| std::fs::read(p).expect("page")).collect();
    let mock = MockScanner::start(&format!("127.0.0.1:{port}"), pages).expect("listen");
    println!("serving {}", mock.url);
    loop {
        std::thread::park();
    }
}
