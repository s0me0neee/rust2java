use crate::model::{Member, Role};

pub struct Group {
    pub role: Role,
    pub members: usize,
    pub total: i32,
    pub best: i32,
}

pub fn group_by_role(members: &[Member]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for role in [Role::Dev, Role::Ops, Role::Design] {
        let selected: Vec<&Member> = members.iter().filter(|m| m.role == role).collect();
        if selected.is_empty() {
            continue;
        }
        let total: i32 = selected.iter().map(|m| m.total()).sum();
        let best = selected.iter().filter_map(|m| m.best()).max().unwrap_or(0);
        groups.push(Group {
            role,
            members: selected.len(),
            total,
            best,
        });
    }
    groups
}

pub fn strongest(members: &[Member]) -> Option<&Member> {
    let mut best: Option<&Member> = None;
    for member in members {
        let better = match best {
            Some(current) => member.total() > current.total(),
            None => true,
        };
        if better {
            best = Some(member);
        }
    }
    best
}

pub fn table(groups: &[Group]) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{:<8} {:>5} {:>7} {:>6} {:>8}",
        "role", "count", "total", "best", "mean"
    ));
    let mut overall = 0;
    for group in groups {
        let mean = group.total as f64 / group.members as f64;
        overall += group.total;
        lines.push(format!(
            "{:<8} {:>5} {:>7} {:>6} {:>8.2}",
            group.role.label(),
            group.members,
            group.total,
            group.best,
            mean
        ));
    }
    lines.push(format!("{:<8} {:>5} {:>7}", "all", "", overall));
    lines.join("\n")
}
