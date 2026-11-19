// Simulation with iter_mut over structs, &mut self methods, retain on structs, nested modules,
// closures capturing &mut, sort_by with floats, and a simple deterministic RNG.
mod rng {
    pub struct Lcg {
        state: u64,
    }

    impl Lcg {
        pub fn new(seed: u64) -> Self {
            Lcg { state: seed }
        }
        pub fn next_u32(&mut self) -> u32 {
            self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.state >> 33) as u32
        }
        pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
            lo + (self.next_u32() % (hi - lo) as u32) as i32
        }
        pub fn unit(&mut self) -> f64 {
            self.next_u32() as f64 / u32::MAX as f64
        }
    }
}

mod physics {
    #[derive(Debug, Clone)]
    pub struct Particle {
        pub id: usize,
        pub x: f64,
        pub y: f64,
        pub vx: f64,
        pub vy: f64,
        pub alive: bool,
    }

    impl Particle {
        pub fn step(&mut self, dt: f64, gravity: f64) {
            self.vy -= gravity * dt;
            self.x += self.vx * dt;
            self.y += self.vy * dt;
            if self.y < 0.0 {
                self.y = -self.y;
                self.vy = -self.vy * 0.5;
            }
        }
        pub fn speed(&self) -> f64 {
            (self.vx * self.vx + self.vy * self.vy).sqrt()
        }
    }

    pub mod bounds {
        pub const WIDTH: f64 = 50.0;
        pub fn inside(x: f64) -> bool {
            (0.0..WIDTH).contains(&x)
        }
    }
}

use physics::{bounds, Particle};

struct World {
    particles: Vec<Particle>,
    tick: u32,
    escaped: Vec<usize>,
}

impl World {
    fn new(n: usize, rng: &mut rng::Lcg) -> Self {
        let particles = (0..n)
            .map(|id| Particle {
                id,
                x: rng.range(5, 45) as f64,
                y: rng.range(0, 20) as f64,
                vx: rng.unit() * 20.0 - 10.0,
                vy: rng.unit() * 10.0,
                alive: true,
            })
            .collect();
        World { particles, tick: 0, escaped: Vec::new() }
    }

    fn step(&mut self) {
        self.tick += 1;
        for p in self.particles.iter_mut() {
            p.step(0.1, 9.8);
            if !bounds::inside(p.x) {
                p.alive = false;
            }
        }
        let gone: Vec<usize> = self.particles.iter().filter(|p| !p.alive).map(|p| p.id).collect();
        self.escaped.extend(gone);
        self.particles.retain(|p| p.alive);
    }

    fn fastest(&self) -> Option<&Particle> {
        self.particles.iter().max_by(|a, b| a.speed().partial_cmp(&b.speed()).unwrap())
    }
}

fn main() {
    let mut rng = rng::Lcg::new(42);
    let rolls: Vec<i32> = (0..8).map(|_| rng.range(1, 7)).collect();
    println!("{:?}", rolls);
    let mut world = World::new(12, &mut rng);
    let mut log = Vec::new();
    let mut record = |w: &World| log.push(format!("t={} n={}", w.tick, w.particles.len()));
    for _ in 0..30 {
        world.step();
        if world.tick % 10 == 0 {
            record(&world);
        }
    }
    println!("{}", log.join(" | "));
    println!("escaped {:?}", world.escaped);
    if let Some(p) = world.fastest() {
        println!("fastest #{} speed {:.3} at ({:.2}, {:.2})", p.id, p.speed(), p.x, p.y);
    }
    let mut by_height: Vec<&Particle> = world.particles.iter().collect();
    by_height.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap().then_with(|| a.id.cmp(&b.id)));
    let ids: Vec<String> = by_height.iter().take(3).map(|p| format!("{}:{:.1}", p.id, p.y)).collect();
    println!("{}", ids.join(", "));
    let total_energy: f64 = world.particles.iter().map(|p| 0.5 * p.speed().powi(2) + 9.8 * p.y).sum();
    println!("energy {:.4}", total_energy);
    let mut counter = 0;
    let mut bump = |by: i32| counter += by;
    bump(3);
    bump(4);
    println!("counter {} width {}", counter, physics::bounds::WIDTH);
}
