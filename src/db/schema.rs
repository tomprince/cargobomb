// This file can be partially regenerated with `diesel print-schema`

use diesel::prelude::*;

table! {
    experiments (id) {
        id -> Int4,
        name -> Varchar,
        mode -> Varchar,
    }
}

table! {
    crates (id) {
        id -> Int4,
        description -> Jsonb,
    }
}

table! {
    toolchains (id) {
        id -> Int4,
        description -> Jsonb,
    }
}

table! {
    experiment_toolchains (experiment_id, toolchain_id) {
        experiment_id -> Int4,
        toolchain_id -> Int4,
    }
}
joinable! { experiment_toolchains -> experiments (experiment_id) }
joinable! { experiment_toolchains -> toolchains (toolchain_id) }

table! {
    experiment_crates (experiment_id, crate_id) {
        experiment_id -> Int4,
        crate_id -> Int4,
        sha -> Nullable<Varchar>,
    }
}
joinable! { experiment_crates -> experiments (experiment_id) }
joinable! { experiment_crates -> crates (crate_id) }
