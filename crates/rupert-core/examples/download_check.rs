//! Dev tool: download online-only files, then free their space again, and
//! report the provider and download state at each step.
//! Usage: cargo run -p rupert-core --example download_check -- <file>...

use std::path::Path;
use std::time::Instant;

use rupert_core::{Provider, cloud, cloud_state};

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let modified = || std::fs::symlink_metadata(path).and_then(|m| m.modified()).ok();
        println!("{}", path.display());
        println!("  provider: {:?}", Provider::detect(path.parent().unwrap()));
        println!("  before:   {:?}, modified {:?}", cloud_state(path).unwrap(), modified());
        let t = Instant::now();
        cloud::download(path).unwrap();
        println!("  download: {:.1}s → {:?}", t.elapsed().as_secs_f64(), cloud_state(path).unwrap());
        println!("  evict:    {:?}", cloud::evict(path).map_err(|e| e.to_string()));
        println!("  after:    {:?}, modified {:?}", cloud_state(path).unwrap(), modified());
    }
}
