use studio_flagship::DemoDayOrchestrator;

fn main() {
    let report = DemoDayOrchestrator::default().run();
    println!("{}", report.to_json().expect("evidence serializes"));
}
