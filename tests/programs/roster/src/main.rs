mod model;
mod report;

use crate::model::{Member, Role};
use crate::report::{group_by_role, strongest, table};

fn main() {
    let members = vec![
        Member::new(1, "ada", Role::Dev, vec![7, 9, 8]),
        Member::new(2, "grace", Role::Ops, vec![10, 6]),
        Member::new(3, "linus", Role::Dev, vec![5, 5, 5, 5]),
        Member::new(4, "mira", Role::Design, vec![9]),
        Member::new(5, "zoe", Role::Ops, vec![]),
    ];
    for member in &members {
        println!(
            "#{} {:<7} {:<7} total {:>3} avg {:.2} best {:?}",
            member.id,
            member.name,
            member.role.label(),
            member.total(),
            member.average(),
            member.best()
        );
    }
    let groups = group_by_role(&members);
    println!("{}", table(&groups));
    match strongest(&members) {
        Some(member) => println!("strongest: {} with {}", member.name, member.total()),
        None => println!("strongest: none"),
    }
    let total: i32 = members.iter().map(|member| member.total()).sum();
    let scored: usize = members.iter().filter(|member| !member.points.is_empty()).count();
    println!("{} points across {} scored members", total, scored);
    let mut names: Vec<&str> = members.iter().map(|member| member.name.as_str()).collect();
    names.sort();
    println!("roster: {}", names.join(", "));
}
