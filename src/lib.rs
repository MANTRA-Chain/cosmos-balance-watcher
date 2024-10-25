#![forbid(unsafe_code)]
#![deny(
    rust_2018_idioms,
    trivial_casts,
    unused_lifetimes,
    unused_qualifications
)]

pub mod config;
pub mod error;
pub mod handle;
pub mod query;
pub mod telemetry;

pub const DEFAULT_CONFIG_PATH: &str = "chains.toml";
