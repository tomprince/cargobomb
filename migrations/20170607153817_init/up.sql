create table experiments (
  id serial primary key,
  name varchar not null unique,
  mode varchar not null
);
create table toolchains (
  id serial primary key,
  description jsonb not null unique
);
create table crates (
  id serial primary key,
  description jsonb not null unique
);
create table experiment_toolchains (
  experiment_id integer not null references experiments (id),
  toolchain_id integer not null references toolchains (id)
);
create table experiment_crates (
  experiment_id integer not null references experiments (id),
  crate_id integer not null references crates (id)
);
