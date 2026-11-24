#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Allergen {
    Eggs = 1,
    Peanuts = 2,
    Shellfish = 4,
    Strawberries = 8,
    Tomatoes = 16,
    Chocolate = 32,
    Pollen = 64,
    Cats = 128,
}

const ALL: [Allergen; 8] = [
    Allergen::Eggs,
    Allergen::Peanuts,
    Allergen::Shellfish,
    Allergen::Strawberries,
    Allergen::Tomatoes,
    Allergen::Chocolate,
    Allergen::Pollen,
    Allergen::Cats,
];

pub struct Allergies(u32);

impl Allergies {
    pub fn is_allergic_to(&self, a: &Allergen) -> bool {
        self.0 & (*a as u32) != 0
    }

    pub fn allergies(&self) -> Vec<Allergen> {
        ALL.iter().filter(|a| self.is_allergic_to(a)).copied().collect()
    }
}

fn main() {
    for score in [0, 1, 5, 248, 255, 509] {
        let a = Allergies(score);
        println!("{score}: {:?} peanuts={}", a.allergies(), a.is_allergic_to(&Allergen::Peanuts));
    }
}
