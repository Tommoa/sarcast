use diesel::prelude::*;

use crate::schema::*;

mod episode;
pub use episode::*;
mod podcast;
pub use podcast::*;

#[derive(Queryable, Identifiable, AsChangeset, PartialEq)]
#[diesel(table_name = source)]
#[diesel(treat_none_as_null = true)]
#[derive(Debug, Clone)]
/// Diesel Model of the source table.
pub struct Source {
    id: i32,
    uri: String,
    last_modified: Option<String>,
    http_etag: Option<String>,
}
