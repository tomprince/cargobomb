use diesel::pg::PgConnection;
use diesel::prelude::*;
use errors::*;
use std::env;


pub fn establish_connection() -> Result<PgConnection> {
    let database_url = env::var("DATABASE_URL").chain_err(
        || "DATABASE_URL must be set",
    )?;
    PgConnection::establish(&database_url).chain_err(|| "Error connecting to database.")
}
