use diesel;
use diesel::pg::PgConnection;
use diesel::pg::upsert::*;
use diesel::prelude::*;
use errors::*;
use ex::ExMode;
use ex::Experiment;
use lists::Crate;
use model::Model;
use serde_json;
use std::env;
use toolchain::Toolchain;

pub(crate) mod schema;

pub(crate) fn establish_connection() -> Result<PgConnection> {
    let database_url = env::var("DATABASE_URL").chain_err(
        || "DATABASE_URL must be set",
    )?;
    PgConnection::establish(&database_url).chain_err(|| "Error connecting to database.")
}


pub struct DbStore {
    conn: PgConnection,
}

impl DbStore {
    pub fn open() -> Result<DbStore> {
        Ok(DbStore { conn: establish_connection()? })
    }
}

impl Model for DbStore {
    fn load_experiment(&self, ex_name: &str) -> Result<Experiment> {
        use db::schema::*;

        let ex: queries::Experiment = experiments::table
            .filter(experiments::name.eq(ex_name))
            .get_result(&self.conn)?;
        let tcs = toolchains::table
            .inner_join(experiment_toolchains::table)
            .select(toolchains::description)
            .filter(experiment_toolchains::experiment_id.eq(ex.id))
            .load(&self.conn)?
            .into_iter()
            .map(|desc| serde_json::from_value(desc).map_err(From::from))
            .collect::<Result<_>>()?;
        let crates = crates::table
            .inner_join(experiment_crates::table)
            .select(crates::description)
            .filter(experiment_crates::experiment_id.eq(ex.id))
            .load(&self.conn)?
            .into_iter()
            .map(|desc| serde_json::from_value(desc).map_err(From::from))
            .collect::<Result<_>>()?;
        Ok(Experiment {
            name: ex.name,
            mode: ex.mode.parse()?,
            toolchains: tcs,
            crates: crates,
        })
    }
    fn create_experiment(
        &self,
        ex_name: &str,
        tcs: Vec<Toolchain>,
        crates: Vec<Crate>,
        mode: ExMode,
    ) -> Result<()> {
        info!(
            "defining experiment {} for {} crates",
            ex_name,
            crates.len()
        );

        use db::schema::*;

        let experiment_id = diesel::insert(&queries::ExperimentInsert {
            name: ex_name.to_string(),
            mode: mode.to_str().to_string(),
        }).into(experiments::table)
            .returning(experiments::id)
            .get_result::<i32>(&self.conn)?;
        let crates = crates
            .into_iter()
            .map(|c| {
                queries::Crate { description: serde_json::to_value(c).unwrap() }
            })
            .collect::<Vec<_>>();
        diesel::insert(&crates.on_conflict_do_nothing())
            .into(crates::table)
            .execute(&self.conn)?;
        let crate_ids = crates::table
            .filter(crates::description.eq_any(
                crates.into_iter().map(|c| c.description),
            ))
            .select(crates::id)
            .load::<i32>(&self.conn)?;
        diesel::insert(&crate_ids
            .into_iter()
            .map(|crate_id| {
                queries::ExperimentCrate {
                    experiment_id,
                    crate_id,
                }
            })
            .collect::<Vec<_>>()).into(experiment_crates::table)
            .execute(&self.conn)?;
        let tcs = tcs.into_iter()
            .map(|tc| {
                queries::Toolchain { description: serde_json::to_value(tc).unwrap() }
            })
            .collect::<Vec<_>>();
        diesel::insert(&tcs.on_conflict_do_nothing())
            .into(toolchains::table)
            .execute(&self.conn)?;
        let toolchain_ids = toolchains::table
            .filter(toolchains::description.eq_any(tcs.into_iter().map(
                |tc| tc.description,
            )))
            .select(toolchains::id)
            .load::<i32>(&self.conn)?;
        diesel::insert(&toolchain_ids
            .into_iter()
            .map(|toolchain_id| {
                queries::ExperimentToolchain {
                    experiment_id,
                    toolchain_id,
                }
            })
            .collect::<Vec<_>>()).into(experiment_toolchains::table)
            .execute(&self.conn)?;

        Ok(())
    }
}

mod queries {
    use db::schema::*;
    #[derive(Queryable)]
    pub struct Experiment {
        pub id: i32,
        pub name: String,
        pub mode: String,
    }
    #[derive(Insertable)]
    #[table_name = "experiments"]
    pub struct ExperimentInsert {
        pub name: String,
        pub mode: String,
    }
    #[derive(Insertable, Queryable)]
    #[table_name = "crates"]
    pub struct Crate {
        pub description: ::serde_json::Value,
    }
    #[derive(Insertable, Queryable)]
    #[table_name = "toolchains"]
    pub struct Toolchain {
        pub description: ::serde_json::Value,
    }
    #[derive(Insertable)]
    #[table_name = "experiment_toolchains"]
    pub struct ExperimentToolchain {
        pub experiment_id: i32,
        pub toolchain_id: i32,
    }
    #[derive(Insertable)]
    #[table_name = "experiment_crates"]
    pub struct ExperimentCrate {
        pub experiment_id: i32,
        pub crate_id: i32,
    }
}
