create table experiment_results (
  experiment_id integer not null references experiments (id),
  crate_id integer not null references crates (id),
  toolchain_id integer not null references toolchains (id),
  result varchar(20),
  log_url text,
  primary key (experiment_id, crate_id, toolchain_id)
);