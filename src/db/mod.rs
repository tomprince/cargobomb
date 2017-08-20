use diesel;
use diesel::pg::PgConnection;
use diesel::pg::upsert::*;
use diesel::prelude::*;
use errors::*;
use ex::ExCrate;
use ex::ExMode;
use ex::Experiment;
use lists::Crate;
use model::Model;
use results::TestResult;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use toolchain::Toolchain;

pub(crate) mod schema;

pub(crate) fn establish_connection(database_url: &str) -> Result<PgConnection> {
    PgConnection::establish(database_url).chain_err(|| "Error connecting to database.")
}


pub struct DbStore {
    conn: Mutex<PgConnection>,
}

impl DbStore {
    pub fn open(database_url: &str) -> Result<DbStore> {
        Ok(DbStore {
            conn: Mutex::new(establish_connection(database_url)?),
        })
    }
}

fn get_experiment(conn: &PgConnection, ex_name: &str) -> Result<queries::Experiment> {
    use db::schema::*;
    Ok(experiments::table
        .filter(experiments::name.eq(ex_name))
        .get_result(conn)?)
}

impl Model for DbStore {
    fn load_experiment(&self, ex_name: &str) -> Result<Experiment> {
        use db::schema::*;
        let conn = self.conn.lock().expect("Poisoined lock");

        conn.transaction(|| {
            let ex = get_experiment(&conn, ex_name)?;
            let tcs = toolchains::table
                .inner_join(experiment_toolchains::table)
                .select(toolchains::description)
                .filter(experiment_toolchains::experiment_id.eq(ex.id))
                .load(&*conn)?
                .into_iter()
                .map(|desc| serde_json::from_value(desc).map_err(From::from))
                .collect::<Result<_>>()?;
            let crates = crates::table
                .inner_join(experiment_crates::table)
                .select(crates::description)
                .filter(experiment_crates::experiment_id.eq(ex.id))
                .load(&*conn)?
                .into_iter()
                .map(|desc| serde_json::from_value(desc).map_err(From::from))
                .collect::<Result<_>>()?;
            Ok(Experiment {
                name: ex.name,
                mode: ex.mode.parse()?,
                toolchains: tcs,
                crates: crates,
            })
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
        let conn = self.conn.lock().expect("Poisoined lock");
        conn.transaction(|| {
            let experiment_id = diesel::insert(&queries::ExperimentInsert {
                name: ex_name.to_string(),
                mode: mode.to_str().to_string(),
            }).into(experiments::table)
                .returning(experiments::id)
                .get_result::<i32>(&*conn)?;
            let crates = crates
                .into_iter()
                .map(|c| {
                    queries::Crate {
                        description: serde_json::to_value(c).unwrap(),
                    }
                })
                .collect::<Vec<_>>();
            diesel::insert(&crates.on_conflict_do_nothing())
                .into(crates::table)
                .execute(&*conn)?;
            let crate_ids = crates::table
                .filter(
                    crates::description.eq_any(crates.into_iter().map(|c| c.description)),
                )
                .select(crates::id)
                .load::<i32>(&*conn)?;
            diesel::insert(&crate_ids
                .into_iter()
                .map(|crate_id| {
                    queries::ExperimentCrate {
                        experiment_id,
                        crate_id,
                    }
                })
                .collect::<Vec<_>>()).into(experiment_crates::table)
                .execute(&*conn)?;
            let tcs = tcs.into_iter()
                .map(|tc| {
                    queries::Toolchain {
                        description: serde_json::to_value(tc).unwrap(),
                    }
                })
                .collect::<Vec<_>>();
            diesel::insert(&tcs.on_conflict_do_nothing())
                .into(toolchains::table)
                .execute(&*conn)?;
            let toolchain_ids = toolchains::table
                .filter(
                    toolchains::description.eq_any(tcs.into_iter().map(|tc| tc.description)),
                )
                .select(toolchains::id)
                .load::<i32>(&*conn)?;
            diesel::insert(&toolchain_ids
                .into_iter()
                .map(|toolchain_id| {
                    queries::ExperimentToolchain {
                        experiment_id,
                        toolchain_id,
                    }
                })
                .collect::<Vec<_>>()).into(experiment_toolchains::table)
                .execute(&*conn)?;

            Ok(())
        })
    }
    fn delete_experiment(&self, ex_name: &str) -> Result<()> {
        use db::schema::*;
        let conn = self.conn.lock().expect("Poisoined lock");

        conn.transaction(|| {
            let ex: queries::Experiment = experiments::table
                .filter(experiments::name.eq(ex_name))
                .get_result(&*conn)?;

            diesel::delete(
                experiment_toolchains::table.filter(experiment_toolchains::experiment_id.eq(ex.id)),
            ).execute(&*conn)?;

            diesel::delete(
                experiment_crates::table.filter(experiment_crates::experiment_id.eq(ex.id)),
            ).execute(&*conn)?;

            diesel::delete(experiments::table.filter(experiments::id.eq(ex.id)))
                .execute(&*conn)?;

            Ok(())
        })
    }
    fn write_shas(&self, ex_name: &str, shas: &HashMap<String, String>) -> Result<()> {
        use db::schema::*;
        let conn = self.conn.lock().expect("Poisoined lock");

        conn.transaction(|| {
            let ex = get_experiment(&conn, ex_name)?;

            let crate_ids = crates::table
                .filter(crates::description.eq_any(shas.keys().map(|url| {
                    serde_json::to_value(Crate::Repo { url: url.clone() }).unwrap()
                })))
                .select((crates::id, crates::description))
                .load::<(i32, serde_json::Value)>(&*conn)?;

            for (crate_id, description) in crate_ids {
                if let Crate::Repo { url } = serde_json::from_value(description)? {
                    diesel::update(
                        experiment_crates::table.filter(
                            experiment_crates::crate_id
                                .eq(crate_id)
                                .and(experiment_crates::experiment_id.eq(ex.id)),
                        ),
                    ).set(&queries::CrateSha { sha: &shas[&url] })
                        .execute(&*conn)?;
                }
            }
            Ok(())
        })
    }
    fn read_shas(&self, ex_name: &str) -> Result<HashMap<String, String>> {
        use db::schema::*;
        let conn = self.conn.lock().expect("Poisoined lock");

        conn.transaction(|| {
            let ex = get_experiment(&conn, ex_name)?;
            let crates = crates::table
                .inner_join(experiment_crates::table)
                .filter(experiment_crates::experiment_id.eq(ex.id))
                .select((crates::description, experiment_crates::sha))
                .load::<(serde_json::Value, Option<String>)>(&*conn)?;

            Ok(
                crates
                    .into_iter()
                    .filter_map(|(desc, sha)| {
                        serde_json::from_value(desc).ok().and_then(
                            |desc| if let Crate::Repo { url } = desc {
                                sha.map(|sha| (url, sha))
                            } else {
                                None
                            },
                        )
                    })
                    .collect(),
            )
        })
    }

    #[allow(unused_variables)]
    fn load_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<Option<TestResult>> {
        use db::schema::*;
        let conn = self.conn.lock().expect("Poisoined lock");

        conn.transaction(|| {
            let ex = get_experiment(&conn, ex_name)?;
            let result = experiment_results::table
                .inner_join(toolchains::table)
                .inner_join(experiments::table)
                ;/*
                .inner_join(crates::table)
                .filter(experiments::name.eq(ex_name))
                .filter(crates::description.eq(serde_json::to_value(crate_).unwrap()))
                .filter(toolchains::description.eq(serde_json::to_value(toolchain).unwrap()))
                .get_result::<queries::Result>(&*conn)?;
            Ok(result.result.parse()?)
            */
        })
    }

    #[allow(unused_variables)]
    fn delete_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<()> {
        Err("NOT IMPLEMENTED".into())
    }

    #[allow(unused_variables)]
    fn read_test_log(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<fs::File> {
        Err("NOT IMPLEMENTED".into())
    }
    #[allow(unused_variables)]
    fn delete_all_test_results(&self, ex_name: &str) -> Result<()> {
        Err("NOT IMPLEMENTED".into())
    }
    #[allow(unused_variables)]
    fn record_test_results(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
        f: &mut FnMut() -> Result<TestResult>,
    ) -> Result<TestResult> {
        f()
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

    #[derive(AsChangeset)]
    #[table_name = "experiment_crates"]
    pub struct CrateSha<'a> {
        pub sha: &'a str,
    }

    #[derive(Queryable)]
    pub struct Result {
        pub result: String,
        pub log_url: String,
    }
}
