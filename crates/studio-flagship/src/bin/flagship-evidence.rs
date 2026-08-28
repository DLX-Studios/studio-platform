#![allow(missing_docs)]

use studio_flagship::DemoDayOrchestrator;

fn main() {
    let report = DemoDayOrchestrator.run();
    println!("{}", report.to_json().expect("evidence serializes"));
}
