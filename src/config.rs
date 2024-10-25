//! Chain configuration
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fs, fs::File, io::Write, path::Path, time::Duration};
use tendermint_rpc::Url;

use crate::error::Error;

pub mod default {
    use super::*;

    pub fn refresh() -> Duration {
        Duration::from_secs(120)
    }

    pub fn coin_type() -> CoinType {
        CoinType::COSMOS
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub prometheus: PrometheusConfig,
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub chains: Vec<ChainConfig>,
}

impl Config {
    pub fn chains_map(&self) -> HashMap<&String, &ChainConfig> {
        self.chains.iter().map(|c| (&c.id, c)).collect()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrometheusConfig {
    pub host: String,
    pub port: i32,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub reset: Option<Duration>,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9090,
            reset: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChainConfig {
    pub id: String,
    pub grpc_addr: Option<Url>,
    pub evm_addr: Option<Url>,
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<Address>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    pub address: String,
    pub hex_address: Option<String>,
    pub role: String,
    pub min_balance: String,
    pub denom: String,
    pub display_denom: Option<String>,
    pub decimal_place: Option<u32>,
    pub balance_url: Option<String>,
    #[serde(default = "default::coin_type")]
    pub coin_type: CoinType,
    #[serde(default = "default::refresh", with = "humantime_serde")]
    pub refresh: Duration,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum CoinType {
    COSMOS,
    EVM,
}

/// Attempt to load and parse the TOML config file as a `Config`.
pub fn load(path: impl AsRef<Path>) -> Result<Config, Error> {
    let config_toml = fs::read_to_string(&path).map_err(Error::config_io)?;

    let config = toml::from_str::<Config>(&config_toml[..]).map_err(Error::config_decode)?;
    check_parse_u128(config.clone())?;
    Ok(config)
}

// Attempt to parse min_balance to u128 as min_balance is String toml while toml not support u128
pub fn check_parse_u128(config: Config) -> Result<(), Error> {
    for chain_config in config.chains.iter() {
        for chain_address in chain_config.addresses.iter() {
            chain_address
                .min_balance
                .parse::<u128>()
                .map_err(Error::config_parse_u128)?;
        }
    }
    Ok(())
}

/// Serialize the given `Config` as TOML to the given config file.
pub fn store(config: &Config, path: impl AsRef<Path>) -> Result<(), Error> {
    let mut file = if path.as_ref().exists() {
        fs::OpenOptions::new().write(true).truncate(true).open(path)
    } else {
        File::create(path)
    }
    .map_err(Error::config_io)?;

    store_writer(config, &mut file)
}

/// Serialize the given `Config` as TOML to the given writer.
pub(crate) fn store_writer(config: &Config, mut writer: impl Write) -> Result<(), Error> {
    let toml_config = toml::to_string_pretty(&config).map_err(Error::config_encode)?;

    writeln!(writer, "{}", toml_config).map_err(Error::config_io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load, store_writer};
    use test_log::test;

    #[test]
    fn parse_valid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains.toml"
        );

        let config = load(path);
        println!("{:?}", config);
        assert!(config.is_ok());
    }

    #[test]
    fn parse_invalid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains-fail.toml"
        );

        let config = load(path);
        println!("{:?}", config);
        assert!(config.is_err());
    }

    #[test]
    fn serialize_valid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains.toml"
        );

        let config = load(path).expect("could not parse config");

        let mut buffer = Vec::new();
        store_writer(&config, &mut buffer).unwrap();
    }
}
