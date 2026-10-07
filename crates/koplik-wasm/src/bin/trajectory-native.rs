//! Offline native half of the cross-target determinism gate.
use koplik_wasm::trajectory_json;
use std::io::Read;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: trajectory-native <scenario.json>");
    let input = if path == "-" {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .expect("read scenario JSON");
        input
    } else {
        std::fs::read_to_string(path).expect("read committed scenario fixture")
    };
    println!("{}", trajectory_json(&input, 0).expect("valid scenario"));
}
