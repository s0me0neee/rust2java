pub struct Member {
    pub id: u32,
    pub name: String,
    pub role: Role,
    pub points: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Dev,
    Ops,
    Design,
}

impl Role {
    pub fn label(&self) -> &'static str {
        match self {
            Role::Dev => "dev",
            Role::Ops => "ops",
            Role::Design => "design",
        }
    }
}

impl Member {
    pub fn new(id: u32, name: &str, role: Role, points: Vec<i32>) -> Member {
        Member {
            id,
            name: name.to_string(),
            role,
            points,
        }
    }

    pub fn total(&self) -> i32 {
        self.points.iter().sum()
    }

    pub fn average(&self) -> f64 {
        if self.points.is_empty() {
            0.0
        } else {
            self.total() as f64 / self.points.len() as f64
        }
    }

    pub fn best(&self) -> Option<i32> {
        self.points.iter().copied().max()
    }
}
