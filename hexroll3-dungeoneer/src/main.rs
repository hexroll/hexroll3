// use rand::SeedableRng;
// use rand::rngs::StdRng;

mod cave;

fn main() {
    // let mut rng = StdRng::seed_from_u64(121135);
    // let mut rng = StdRng::seed_from_u64(134416);
    // let mut rng = StdRng::seed_from_u64(30123);
    let mut rng = rand::thread_rng();
    let cave = {
        let mut attempts = 10;
        let mut cave = cave::CaveBuilder::new(&mut rng, false);
        while attempts > 0 && cave.caverns.len() < 5 {
            cave = cave::CaveBuilder::new(&mut rng, false);
            attempts -= 1;
        }
        cave
    };
    println!("{}", cave.as_json());
}
