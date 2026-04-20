use rand::SeedableRng;
use rand::rngs::StdRng;

mod cave;

fn main() {
    // let mut rng = StdRng::seed_from_u64(121135);
    let mut rng = StdRng::seed_from_u64(3123416);
    let cave = cave::CaveBuilder::new(&mut rng, false);
    println!("{}", cave.as_json());
}
