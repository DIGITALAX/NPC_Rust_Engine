use std::f64::consts::PI;

fn between(min: i32, max: i32) -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * (PI / 180.0)
}

pub fn distance_between(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    (dx * dx + dy * dy).sqrt()
}

pub fn rad_to_deg(radians: f64) -> f64 {
    radians * (180.0 / PI)
}
