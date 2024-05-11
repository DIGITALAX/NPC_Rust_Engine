use rand::Rng;

use crate::Coordenada;

pub fn between(min: f32, max: f32) -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}


pub fn distance(a: &Coordenada, b: &Coordenada) -> f32 {
    ((a.x - b.x).abs() + (a.y - b.y).abs()) as f32
}
