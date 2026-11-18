// Struct value semantics through `&mut` and `Copy`, a module named like its struct, Debug builders.
use std::collections::BTreeMap;
use std::fmt;

mod vec2 {
    #[derive(Clone, Copy, PartialEq)]
    pub struct Vec2 {
        pub x: f64,
        pub y: f64,
    }

    impl Vec2 {
        pub fn new(x: f64, y: f64) -> Self {
            Self { x, y }
        }
        pub fn reset(&mut self) {
            *self = Vec2::new(0.0, 0.0);
        }
    }

    impl std::fmt::Debug for Vec2 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Vec2").field("x", &self.x).field("y", &self.y).finish()
        }
    }
}

use vec2::Vec2;

#[derive(Clone, Copy, Debug)]
struct Body {
    pos: Vec2,
    mass: u32,
}

struct Pair(i32, &'static str);
struct Bag {
    items: Vec<i32>,
    tags: BTreeMap<String, u8>,
}
struct Hidden {
    id: u32,
}
struct Unit;

impl fmt::Debug for Pair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Pair").field(&self.0).field(&self.1).finish()
    }
}
impl fmt::Debug for Bag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.items.iter()).finish()?;
        f.debug_map().entries(self.tags.iter()).finish()
    }
}
impl fmt::Debug for Hidden {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Hidden").field("id", &self.id).finish_non_exhaustive()
    }
}
impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Unit").finish()
    }
}

fn swap_xy(p: &mut Vec2) {
    *p = Vec2::new(p.y, p.x);
}

fn main() {
    // `*p = v` through a parameter, `iter_mut`, and `*self`.
    let mut a = Vec2::new(1.0, 2.0);
    swap_xy(&mut a);
    let mut points = vec![Vec2::new(1.0, 1.0), Vec2::new(2.0, 3.0)];
    for p in points.iter_mut() {
        *p = Vec2::new(p.x * 10.0, p.y);
    }
    points[1].reset();
    println!("{:?} {:?}", a, points);

    // Copies of a Copy struct that is changed in place stay independent.
    let b = Body { pos: Vec2::new(1.0, 1.0), mass: 3 };
    let mut c = b;
    c.mass = 9;
    c.pos.x = 7.0;
    let mut bodies = vec![b, c];
    bodies[0].pos = Vec2::new(5.0, 5.0);
    println!("{:?} {:?} {:?}", b, c, bodies);
    println!("{}", b.pos == Vec2::new(1.0, 1.0));

    let bag = Bag { items: vec![3, 1], tags: BTreeMap::from([("k".to_string(), 2)]) };
    println!("{:?} | {:?} | {:?} | {:?}", Pair(4, "four"), bag, Hidden { id: 7 }, Unit);
}
