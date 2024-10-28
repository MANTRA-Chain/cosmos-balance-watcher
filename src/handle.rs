use crate::config;
use crate::telemetry::{
    ACCOUNT_BALANCE_COLLECTOR, ACCOUNT_QUERY_STATUS_COLLECTOR, ACCOUNT_STATUS_COLLECTOR,
    account_balance_setter, account_status_setter, account_query_status_setter
};
use log::{error, info, warn};
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

pub async fn track_account_status(
    grpc_addr: Option<Url>,
    evm_addr: Option<Url>,
    chain_id: String,
    chain_address: config::Address,
) {
    let address = &chain_address.address;
    let hex_address = &chain_address.hex_address;
    let denom = &chain_address.denom;
    let display_denom = &chain_address.display_denom;
    let decimal_place = &chain_address.decimal_place;
    let refresh = &chain_address.refresh;
    let balance_url = &chain_address.balance_url;
    let role = &chain_address.role;
    let min_balance = &chain_address.min_balance;
    let coin_type = &chain_address.coin_type;
    let display_min_balance = if let Some(dp) = decimal_place {
        from_atomics(min_balance, *dp)
    } else {
        min_balance.clone()
    };
    let mut balance: String;
    let mut collect_interval = tokio::time::interval(refresh.to_owned());

    loop {
        collect_interval.tick().await;

        balance = match coin_type
            .get_balance(
                address.into(),
                denom.into(),
                grpc_addr.clone(),
                evm_addr.clone(),
            )
            .await
        {
            Ok(balance) => {
                account_query_status_setter(
                    &chain_id,
                    hex_address.as_ref().unwrap_or(address),
                    display_denom.as_ref().unwrap_or(denom),
                    &display_min_balance,
                    role,
                    balance_url.as_ref().unwrap_or(&"".to_string()),
                    0,
                );
                balance
            }
            Err(error) => {
                error!("{} and retry next refresh", error);
                account_query_status_setter(
                    &chain_id,
                    hex_address.as_ref().unwrap_or(address),
                    display_denom.as_ref().unwrap_or(denom),
                    &display_min_balance,
                    role,
                    balance_url.as_ref().unwrap_or(&"".to_string()),
                    1,
                );
                continue;
            }
        };
        info!(
            "The latest balance={}{} with address ({}) for {} on ({})",
            balance, denom, address, role, chain_id
        );
        if balance.parse::<u128>().unwrap() < min_balance.parse::<u128>().unwrap() {
            warn!("The current balance {}{denom} is less than {}{denom} with address ({}) for {} on ({})", balance, min_balance, address, role, chain_id, denom=denom);
            account_status_setter(
                &chain_id,
                hex_address.as_ref().unwrap_or(address),
                display_denom.as_ref().unwrap_or(denom),
                &display_min_balance,
                role,
                balance_url.as_ref().unwrap_or(&"".to_string()),
                1,
            );
        } else {
            account_status_setter(
                &chain_id,
                hex_address.as_ref().unwrap_or(address),
                display_denom.as_ref().unwrap_or(denom),
                &display_min_balance,
                role,
                balance_url.as_ref().unwrap_or(&"".to_string()),
                0,
            );
        }

        if !chain_address.disable_balance {
            if let Some(dp) = decimal_place {
                let display_balance = from_atomics(&balance, *dp);
                account_balance_setter(
                    &chain_id,
                    hex_address.as_ref().unwrap_or(address),
                    display_denom.as_ref().unwrap_or(denom),
                    &display_min_balance,
                    role,
                    balance_url.as_ref().unwrap_or(&"".to_string()),
                    display_balance.parse::<i64>().unwrap(),
                );
            }
        }
    }
}

fn from_atomics(number: &String, decimal_place: u32) -> String {
    let base = 10u128;
    let divisor = base.checked_pow(decimal_place).unwrap();
    number
        .clone()
        .parse::<u128>()
        .unwrap()
        .checked_div(divisor)
        .unwrap_or_default()
        .to_string()
}
