use std::sync::LazyLock;

use rusqlite_migration::{M, Migrations};

/// The forward-only migration ladder. Shipped entries are immutable — a change
/// to the schema is always a *new* `M::up(...)` appended here, never an edit to
/// an existing one. See the `add-db-migration` skill.
pub static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(include_str!("migrations/001_initial.sql"))]));

#[cfg(test)]
mod tests {
    use super::*;

    /// Mandatory per `.claude/rules/testing.md`: the whole ladder applies cleanly
    /// on a fresh in-memory DB.
    #[test]
    fn migrations_apply_on_fresh_memory_db() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
    }

    #[test]
    fn migrations_validate() {
        MIGRATIONS.validate().unwrap();
    }
}
