//! Dev tool: scan from an eSCL scanner (by address) into a folder.
//! Usage: cargo run -p rupert-core --example scan_check -- <address> <dest-dir> [feeder|glass]

use std::path::PathBuf;
use std::time::Instant;

use rupert_core::escl::{self, ColorMode, InputSource, PageSize, ScanRequest};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let scanner = escl::scanner_at(&a[0]);
    let caps = escl::capabilities(&scanner).expect("capabilities");
    println!(
        "{:?}: platen {:?}, feeder {:?}, duplex {}",
        caps.make_and_model,
        caps.platen.as_ref().map(|p| &p.resolutions),
        caps.feeder.as_ref().map(|f| &f.resolutions),
        caps.duplex
    );
    let source = if a.get(2).map(String::as_str) == Some("glass") {
        InputSource::Platen
    } else {
        InputSource::Feeder
    };
    let req = ScanRequest {
        source,
        duplex: false,
        color: ColorMode::Gray,
        resolution: 150,
        page_size: PageSize::Letter,
    };
    let t = Instant::now();
    let path = escl::scan(
        &scanner,
        &caps,
        &req,
        &PathBuf::from(&a[1]),
        "Scan test",
        |n| println!("  page {n}"),
    )
    .expect("scan");
    println!(
        "saved {} in {:.1}s",
        path.display(),
        t.elapsed().as_secs_f64()
    );
}
