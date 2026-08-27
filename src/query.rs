use crate::config::CoinType;
use crate::handle::CoinEntity;
use anyhow::{anyhow, Result};
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
use log::warn;
use serde_json::{from_slice, to_vec};
use std::future::Future;
use std::str::FromStr;
use tendermint_rpc::Url;
use web3::contract::{Contract, Options};
use web3::types::Address;

/// Tries each endpoint in order, falling back to the next one on any error.
/// Returns the successful value along with the endpoint that produced it.
async fn try_endpoints<T, F, Fut>(endpoints: &[Url], mut op: F) -> Result<(T, String)>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_err = None;
    let last_index = endpoints.len().saturating_sub(1);
    for (i, endpoint) in endpoints.iter().enumerate() {
        let endpoint_str = endpoint.to_string();
        match op(endpoint_str.clone()).await {
            Ok(value) => return Ok((value, endpoint_str)),
            Err(e) => {
                if i < last_index {
                    warn!(
                        "query failed on endpoint {}: {}, trying next endpoint",
                        endpoint_str, e
                    );
                } else {
                    warn!("query failed on endpoint {}: {}", endpoint_str, e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no endpoint configured")))
}

fn join_endpoints(endpoints: &[Url]) -> String {
    endpoints
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

impl CoinType {
    pub async fn get_balance(
        &self,
        address: String,
        denom: String,
        contract_address: Option<String>,
        grpc_addrs: Vec<Url>,
        evm_addrs: Vec<Url>,
    ) -> Result<String> {
        match self {
            CoinType::COSMOS => try_endpoints(&grpc_addrs, |endpoint| {
                get_cosmos_balance(address.clone(), denom.clone(), endpoint)
            })
            .await
            .map(|(balance, _)| balance),
            CoinType::CW20 => {
                let contract_address = contract_address.unwrap();
                try_endpoints(&grpc_addrs, |endpoint| {
                    get_cw20_balance(address.clone(), contract_address.clone(), endpoint)
                })
                .await
                .map(|(balance, _)| balance)
            }
            CoinType::EVM => try_endpoints(&evm_addrs, |endpoint| {
                get_evm_balance(address.clone(), endpoint)
            })
            .await
            .map(|(balance, _)| balance),
            CoinType::EVM_ERC20 => {
                let contract_address = contract_address.unwrap();
                try_endpoints(&evm_addrs, |endpoint| {
                    get_evm_erc20_balance(address.clone(), contract_address.clone(), endpoint)
                })
                .await
                .map(|(balance, _)| balance)
            }
        }
    }
    pub async fn get_balances(
        &self,
        address: String,
        coin_entities: &[CoinEntity],
        grpc_addrs: Vec<Url>,
        evm_addrs: Vec<Url>,
    ) -> Result<(Vec<Coin>, String)> {
        // `query_endpoint_url` is reported as the full configured endpoint list
        // (not just whichever one happened to answer) so the account_query_status
        // gauge keeps a single stable label series per address/coin_type, on both
        // success and failure, instead of leaving stale series behind whenever
        // fallback switches endpoints.
        match self {
            CoinType::COSMOS => try_endpoints(&grpc_addrs, |endpoint| {
                get_cosmos_balances(address.clone(), endpoint)
            })
            .await
            .map(|(balances, _)| (balances, join_endpoints(&grpc_addrs)))
            .map_err(|e| {
                crate::error::Error::query_error(e.to_string(), join_endpoints(&grpc_addrs)).into()
            }),
            CoinType::CW20 => try_endpoints(&grpc_addrs, |endpoint| {
                let address = address.clone();
                async move {
                    let mut coins = Vec::<Coin>::new();
                    for coin_entity in coin_entities {
                        let contract_address = coin_entity.contract_address.clone().unwrap();
                        let balance =
                            get_cw20_balance(address.clone(), contract_address, endpoint.clone())
                                .await?;
                        coins.push(Coin {
                            denom: coin_entity.denom.clone(),
                            amount: balance,
                        });
                    }
                    Ok(coins)
                }
            })
            .await
            .map(|(coins, _)| (coins, join_endpoints(&grpc_addrs)))
            .map_err(|e| {
                crate::error::Error::query_error(e.to_string(), join_endpoints(&grpc_addrs)).into()
            }),
            CoinType::EVM => {
                let denom = coin_entities.first().unwrap().denom.clone();
                try_endpoints(&evm_addrs, |endpoint| {
                    get_evm_balance(address.clone(), endpoint)
                })
                .await
                .map(|(balance, _)| {
                    (
                        vec![Coin {
                            denom,
                            amount: balance,
                        }],
                        join_endpoints(&evm_addrs),
                    )
                })
                .map_err(|e| {
                    crate::error::Error::query_error(e.to_string(), join_endpoints(&evm_addrs))
                        .into()
                })
            }
            CoinType::EVM_ERC20 => try_endpoints(&evm_addrs, |endpoint| {
                let address = address.clone();
                async move {
                    let mut coins = Vec::<Coin>::new();
                    for coin_entity in coin_entities {
                        let contract_address = coin_entity.contract_address.clone().unwrap();
                        let balance = get_evm_erc20_balance(
                            address.clone(),
                            contract_address,
                            endpoint.clone(),
                        )
                        .await?;
                        coins.push(Coin {
                            denom: coin_entity.denom.clone(),
                            amount: balance,
                        });
                    }
                    Ok(coins)
                }
            })
            .await
            .map(|(coins, _)| (coins, join_endpoints(&evm_addrs)))
            .map_err(|e| {
                crate::error::Error::query_error(e.to_string(), join_endpoints(&evm_addrs)).into()
            }),
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

/// Fetches ERC-20 token balance via `balanceOf(address)` eth_call.
/// Works for any EVM ERC-20 token (MantraUSD, USDC, USDT, etc.).
pub async fn get_evm_erc20_balance(
    address: String,
    contract_address: String,
    evm_addr: String,
) -> Result<String> {
    // Minimal ERC-20 ABI — only balanceOf is needed
    let abi = r#"[{
        "constant": true,
        "inputs": [{"name": "_owner", "type": "address"}],
        "name": "balanceOf",
        "outputs": [{"name": "balance", "type": "uint256"}],
        "type": "function"
    }]"#;

    let transport = web3::transports::Http::new(&evm_addr)?;
    let web3 = web3::Web3::new(transport);

    let contract_addr = Address::from_str(&contract_address)?;
    let wallet_addr = Address::from_str(&address)?;

    let contract = Contract::from_json(web3.eth(), contract_addr, abi.as_bytes())?;

    let balance: web3::types::U256 = contract
        .query("balanceOf", (wallet_addr,), None, Options::default(), None)
        .await?;

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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[actix_rt::test]
    async fn test_try_endpoints_falls_back_on_error() {
        let endpoints: Vec<Url> = vec![
            "http://127.0.0.1:1".parse().unwrap(),
            "http://127.0.0.1:2".parse().unwrap(),
        ];
        let attempts = AtomicUsize::new(0);

        let (value, used) = try_endpoints(&endpoints, |endpoint| {
            let attempts = &attempts;
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                if endpoint.contains(":1") {
                    Err(anyhow!("endpoint {} down", endpoint))
                } else {
                    Ok(endpoint)
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(value, "http://127.0.0.1:2/");
        assert_eq!(used, "http://127.0.0.1:2/");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[actix_rt::test]
    async fn test_try_endpoints_returns_err_when_all_fail() {
        let endpoints: Vec<Url> = vec![
            "http://127.0.0.1:1".parse().unwrap(),
            "http://127.0.0.1:2".parse().unwrap(),
        ];
        let attempts = AtomicUsize::new(0);

        let result: Result<((), String)> = try_endpoints(&endpoints, |_endpoint| {
            let attempts = &attempts;
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("always down"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

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
        let evm_addr = "https://ethereum-rpc.publicnode.com".to_string();
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

    #[actix_rt::test]
    async fn test_get_evm_erc20_balance() {
        // MantraUSD (6 decimals) on MANTRA Mainnet — stablebridge treasury wallet
        let address = "0x83526104bd67b8b230685dcc38129b7c0fc8c340".to_string();
        let contract_address = "0xd2b95283011E47257917770D28Bb3EE44c849f6F".to_string();
        let evm_addr = "https://evm.mantrachain.io".to_string();
        let balance = get_evm_erc20_balance(address, contract_address, evm_addr)
            .await
            .unwrap();
        println!("MantraUSD raw balance: {}", balance);
        let balance_u128: u128 = balance.parse().unwrap();
        // Treasury must hold at least 100 MantraUSD (100 * 10^6) to keep bridge running
        assert!(
            balance_u128 > 0,
            "treasury MantraUSD balance should be non-zero"
        );
    }
}
