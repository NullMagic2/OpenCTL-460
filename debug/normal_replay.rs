//! Replay a recorded capture through the current normal processor, without opening a device.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
fn main() {
    let input = std::fs::read_to_string(std::env::args().nth(1).expect("trace path")).unwrap();
    let config = input
        .split("# config-begin\n")
        .nth(1)
        .unwrap()
        .split("# config-end")
        .next()
        .unwrap()
        .lines()
        .map(|l| l.strip_prefix("# ").unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut engine = Engine::new(Config::parse(&config).unwrap()).unwrap();
    println!("sequence,time,raw_x,raw_y,old_x,old_y,new_x,new_y,contact");
    for line in input
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("sequence") && !l.is_empty())
    {
        let v: Vec<_> = line.split(',').collect();
        let n = |i: usize| v[i].parse::<u16>().unwrap();
        let flag = |i: usize| v[i] == "1";
        let time = v[1].parse::<f64>().unwrap();
        engine
            .push(
                Sample {
                    x: n(3),
                    y: n(4),
                    pressure: n(5),
                    in_range: flag(6),
                    position_valid: flag(7),
                    tip: flag(8),
                    barrel: flag(12),
                    eraser: flag(13),
                    ..Default::default()
                },
                time,
            )
            .unwrap();
        let f = engine.tick(time);
        println!(
            "{},{},{},{},{},{},{},{},{}",
            v[0],
            v[1],
            v[3],
            v[4],
            v[14],
            v[15],
            f.x,
            f.y,
            u8::from(f.contact)
        );
    }
}
