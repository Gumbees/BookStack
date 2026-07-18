pub mod auth_providers;
pub mod books;
pub mod chapters;
pub mod comments;
pub mod directory;
pub mod exports;
pub mod oauth;
pub mod orgs;
pub mod pages;
pub mod recycle;
pub mod search;
pub mod settings;
pub mod shelves;
pub mod system;
pub mod tags;
pub mod users;

use crate::CoreError;

/// Map a unique-constraint violation to a friendly validation error.
pub(crate) fn map_unique(err: sqlx::Error, message: &str) -> CoreError {
    if let sqlx::Error::Database(ref db) = err {
        if db.is_unique_violation() {
            return CoreError::validation(message);
        }
    }
    CoreError::from(err)
}
