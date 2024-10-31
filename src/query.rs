use crate::config::CoinType;
use crate::handle::CoinEntity;
use anyhow::Result;
use cosmos_sdk_proto::cosmos::bank::v1beta1::{
    query_client::QueryClient, QueryAllBalancesRequest, QueryBalanceRequest,
};
use cosmos_sdk_proto::cosmos::base::query::v1beta1::PageRequest;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::cosmwasm::wasm::v1::{
    query_client::QueryClient as WasmQueryClient, QuerySmartContractStateRequest,
};
use cw20::{BalanceResponse, Cw20QueryMsg::Balance};
use http::uri::Uri;
use serde_json::{from_slice, to_vec};
use std::str::FromStr;
use tendermint_rpc::Url;
use web3::types::Address;

impl CoinType {
    pub async fn get_balance(
        &self,
        address: String,
        denom: String,
        contract_address: Option<String>,
        grpc_addr: Option<Url>,
        evm_addr: Option<Url>,
    ) -> Result<String> {
        match self {
            CoinType::COSMOS => {
                get_cosmos_balance(address, denom, grpc_addr.unwrap().to_string()).await
            }
            CoinType::CW20 => {
                get_cw20_balance(
                    address,
                    contract_address.unwrap().to_string(),
                    grpc_addr.unwrap().to_string(),
                )
                .await
            }
            CoinType::EVM => get_evm_balance(address, evm_addr.unwrap().to_string()).await,
        }
    }
    pub async fn get_balances(
        &self,
        address: String,
        coin_entities: &[CoinEntity],
        grpc_addr: Option<Url>,
        evm_addr: Option<Url>,
    ) -> Result<(Vec<Coin>, String)> {
        match self {
            CoinType::COSMOS => {
                let grpc_addr = grpc_addr.unwrap().to_string();
                get_cosmos_balances(address, grpc_addr.clone())
                    .await
                    .map(|balances| (balances, grpc_addr.clone()))
                    .map_err(|e| crate::error::Error::query_error(e.to_string(), grpc_addr).into())
            }
            CoinType::CW20 => {
                let grpc_addr = grpc_addr.unwrap().to_string();
                let mut coins = Vec::<Coin>::new();
                for coin_entity in coin_entities {
                    let contract_address = coin_entity.contract_address.clone().unwrap();
                    let balance =
                        get_cw20_balance(address.clone(), contract_address, grpc_addr.clone())
                            .await
                            .map_err(|e| {
                                crate::error::Error::query_error(e.to_string(), grpc_addr.clone())
                            })?;
                    coins.push(Coin {
                        denom: coin_entity.denom.clone(),
                        amount: balance,
                    });
                }
                Ok((coins, grpc_addr))
            }
            CoinType::EVM => {
                let evm_addr = evm_addr.unwrap().to_string();
                let denom = coin_entities.first().unwrap().denom.clone();
                get_evm_balance(address, evm_addr.clone())
                    .await
                    .map(|balance| {
                        (
                            vec![Coin {
                                denom,
                                amount: balance,
                            }],
                            evm_addr.clone(),
                        )
                    })
                    .map_err(|e| crate::error::Error::query_error(e.to_string(), evm_addr).into())
            }
        }
    }
}

/// Fetches on-chain balance of given address and chain
pub async fn get_cosmos_balance(
    address: String,
    denom: String,
    grpc_addr: String,
) -> Result<String> {
    let mut query_client = create_grpc_client(grpc_addr.parse::<Uri>()?, QueryClient::new).await?;
    let request = QueryBalanceRequest {
        address,
        denom: denom.clone(),
    };
    Ok(query_client
        .balance(request)
        .await?
        .into_inner()
        .balance
        .map(|coin| coin.amount)
        .ok_or_else(|| crate::error::Error::get_cosmos_balance(denom))?)
}

/// Fetches on-chain balance of given address and chain
pub async fn get_cosmos_balances(address: String, grpc_addr: String) -> Result<Vec<Coin>> {
    let mut query_client = create_grpc_client(grpc_addr.parse::<Uri>()?, QueryClient::new).await?;

    let mut page_request = PageRequest {
        key: vec![],
        offset: 0,
        limit: 100,
        count_total: true,
        reverse: true,
    };
    let request = QueryAllBalancesRequest {
        address: address.clone(),
        pagination: Some(page_request.clone()),
        ..Default::default()
    };

    let mut coins = Vec::<Coin>::new();

    let mut response = query_client.all_balances(request).await?.into_inner();

    coins.extend(response.balances);

    while let Some(pagination) = response.pagination {
        if pagination.next_key.is_empty() {
            break;
        }
        page_request.key = pagination.next_key;
        let request = QueryAllBalancesRequest {
            address: address.clone(),
            pagination: Some(page_request.clone()),
            ..Default::default()
        };
        response = query_client.all_balances(request).await?.into_inner();
        coins.extend(response.balances);
    }

    Ok(coins)
}

pub async fn get_evm_balance(address: String, evm_addr: String) -> Result<String> {
    let transport = web3::transports::Http::new(&evm_addr)?;
    let web3 = web3::Web3::new(transport);
    let account = Address::from_str(&address)?;
    let balance = web3.eth().balance(account, None).await?;
    Ok(balance.as_u128().to_string())
}

pub async fn get_cw20_balance(
    address: String,
    contract_address: String,
    grpc_addr: String,
) -> Result<String> {
    let mut query_client =
        create_grpc_client(grpc_addr.parse::<Uri>()?, WasmQueryClient::new).await?;
    let request = QuerySmartContractStateRequest {
        address: contract_address,
        query_data: to_vec(&Balance { address })?,
    };
    let resp: BalanceResponse = from_slice(
        &query_client
            .smart_contract_state(request)
            .await?
            .into_inner()
            .data,
    )?;
    Ok(resp.balance.to_string())
}

/// Helper function to create a gRPC client.
pub async fn create_grpc_client<T>(
    grpc_addr: Uri,
    client_constructor: impl FnOnce(tonic::transport::Channel) -> T,
) -> Result<T, crate::error::Error> {
    let tls_config = tonic::transport::ClientTlsConfig::new().with_native_roots();
    let channel = tonic::transport::Channel::builder(grpc_addr)
        .tls_config(tls_config)
        .map_err(crate::error::Error::grpc_transport)?
        .connect()
        .await
        .map_err(crate::error::Error::grpc_transport)?;
    Ok(client_constructor(channel))
}

#[cfg(test)]
mod tests {
    use more_asserts::assert_ge;

    use super::*;

    // TODO: use mock server instead
    #[actix_rt::test]
    async fn test_get_cosmos_balance() {
        let address = "mantra1y8hxa8q0qk6h2fxtugkx67re38k03888azp4dg".to_string();
        let denom = "uom".to_string();
        let endpoint_addr = "https://grpc.mantrachain.io".to_string();
        let balance = get_cosmos_balance(address, denom, endpoint_addr)
            .await
            .unwrap();
        println!("{:?}", balance);
        assert_ne!(balance, "".to_string());
    }

    #[actix_rt::test]
    async fn test_get_evm_balance() {
        let address = "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B".to_string(); // vitalik
        let evm_addr = "https://eth.llamarpc.com".to_string();
        let balance = get_evm_balance(address, evm_addr).await.unwrap();
        println!("{:?}", balance);
        assert_ne!(balance, "".to_string());
    }

    #[actix_rt::test]
    async fn test_get_cosmos_balances() {
        let address = "mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm".to_string();
        let endpoint_addr = "https://grpc.dukong.mantrachain.io".to_string();
        let balances = get_cosmos_balances(address, endpoint_addr).await.unwrap();
        println!("{:#?}", balances);
        assert_ge!(balances.len(), 0);
    }

    #[actix_rt::test]
    async fn test_get_cw20_balance() {
        let address = "mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc".to_string();
        let contract_address =
            "mantra1wrvwhcfuhqe7eru59ehkxxr2e262ksnzhtfmdtr96wctr8m2kafq2vh64r".to_string();
        let endpoint_addr = "https://grpc.dukong.mantrachain.io".to_string();
        let balance = get_cw20_balance(address, contract_address, endpoint_addr)
            .await
            .unwrap();
        println!("{:?}", balance);
        assert_ne!(balance, "".to_string());
    }
}
