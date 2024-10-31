use crate::config;
use crate::telemetry::{
    account_balance_setter, account_query_status_setter, account_status_setter,
    ACCOUNT_BALANCE_COLLECTOR, ACCOUNT_QUERY_STATUS_COLLECTOR, ACCOUNT_STATUS_COLLECTOR,
};
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use log::{error, info, warn};
use std::collections::HashMap;
use tendermint_rpc::Url;

pub async fn account_status_collector(config: config::Config) {
    for chain_config in config.chains.iter() {
        let grpc_addr = chain_config.grpc_addr.clone();
        let evm_addr = chain_config.evm_addr.clone();
        let chain_id = chain_config.id.clone();
        for chain_address in chain_config.addresses.clone().iter() {
            tokio::task::spawn(track_account_status(
                grpc_addr.clone(),
                evm_addr.clone(),
                chain_id.clone(),
                chain_address.clone(),
            ));
        }
    }
    if let Some(interval) = config.prometheus.reset {
        let mut reset_interval = tokio::time::interval(interval);
        loop {
            reset_interval.tick().await;
            info!("reset metrics!");
            ACCOUNT_BALANCE_COLLECTOR.reset();
            ACCOUNT_STATUS_COLLECTOR.reset();
            ACCOUNT_QUERY_STATUS_COLLECTOR.reset();
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct CoinEntity {
    pub coin_type: config::CoinType,
    pub contract_address: Option<String>,
    pub decimal_place: u32,
    pub denom: String,
    pub display_denom: String,
    pub display_min_balance: String,
    pub min_balance: String,
}

pub async fn track_account_status(
    grpc_addr: Option<Url>,
    evm_addr: Option<Url>,
    chain_id: String,
    chain_address: config::Address,
) {
    let address = chain_address
        .hex_address
        .unwrap_or(chain_address.address.clone());
    let refresh = &chain_address.refresh;
    let balance_url = &chain_address.balance_url;
    let role = &chain_address.role;
    let mut collect_interval = tokio::time::interval(refresh.to_owned());
    let mut coin_map: HashMap<config::CoinType, Vec<CoinEntity>> = HashMap::new();
    for coin in chain_address.coins.iter() {
        let display_min_balance = from_atomics(&coin.min_balance, coin.decimal_place);
        let coin_entity = CoinEntity {
            coin_type: coin.coin_type.clone(),
            contract_address: coin.contract_address.clone(),
            decimal_place: coin.decimal_place,
            denom: coin.denom.clone(),
            display_denom: coin.display_denom.clone().unwrap_or(coin.denom.clone()),
            display_min_balance,
            min_balance: coin.min_balance.clone(),
        };
        coin_map
            .entry(coin.coin_type.clone())
            .or_default()
            .push(coin_entity);
    }

    loop {
        collect_interval.tick().await;
        for (coin_type, coin_entities) in coin_map.iter() {
            let tmp_balances = match coin_type
                .get_balances(
                    address.clone(),
                    coin_entities,
                    grpc_addr.clone(),
                    evm_addr.clone(),
                )
                .await
            {
                Ok((balances, query_endpoint_url)) => {
                    account_query_status_setter(
                        &chain_id,
                        &address,
                        role,
                        balance_url.as_ref().unwrap_or(&"".to_string()),
                        &query_endpoint_url,
                        0,
                    );
                    balances
                }
                Err(e) => {
                    error!("{} and retry next refresh", e);
                    let error_string = e.to_string();
                    let query_endpoint_url = error_string
                        .split(" (endpoint: ")
                        .last()
                        .unwrap_or("")
                        .split(')')
                        .next()
                        .unwrap_or("");
                    account_query_status_setter(
                        &chain_id,
                        &address,
                        role,
                        balance_url.as_ref().unwrap_or(&"".to_string()),
                        query_endpoint_url,
                        1,
                    );
                    continue;
                }
            };

            for coin_entity in coin_entities {
                let default_coin = Coin {
                    denom: coin_entity.denom.clone(),
                    amount: "0".to_owned(),
                };
                let coin = tmp_balances
                    .iter()
                    .find(|coin| coin.denom == coin_entity.denom)
                    .unwrap_or(&default_coin);
                if coin.amount.parse::<u128>().unwrap()
                    <= coin_entity.min_balance.parse::<u128>().unwrap()
                {
                    warn!("The current balance {}{denom} is less than {}{denom} with address ({}) for {} on ({})", coin.amount, coin_entity.min_balance, address, role, chain_id, denom=coin.denom);

                    account_status_setter(
                        &chain_id,
                        &address,
                        &coin_entity.display_denom,
                        &coin_entity.display_min_balance,
                        role,
                        balance_url.as_ref().unwrap_or(&"".to_string()),
                        1,
                    );
                } else {
                    account_status_setter(
                        &chain_id,
                        &address,
                        &coin_entity.display_denom,
                        &coin_entity.display_min_balance,
                        role,
                        balance_url.as_ref().unwrap_or(&"".to_string()),
                        0,
                    );
                }

                if chain_address.disable_balance != Some(true) {
                    let display_balance = from_atomics(&coin.amount, coin_entity.decimal_place);
                    account_balance_setter(
                        &chain_id,
                        &address,
                        &coin_entity.display_denom,
                        role,
                        balance_url.as_ref().unwrap_or(&"".to_string()),
                        display_balance.parse::<i64>().unwrap(),
                    );
                }
                info!(
                    "The latest balance={}{} with address ({}) for {} on ({})",
                    coin.amount, coin.denom, address, role, chain_id
                );
            }
        }
    }
}

fn from_atomics(number: &str, decimal_place: u32) -> String {
    let base = 10u128;
    let divisor = base.checked_pow(decimal_place).unwrap();
    number
        .parse::<u128>()
        .unwrap()
        .checked_div(divisor)
        .unwrap_or_default()
        .to_string()
}
