use crate::config::CoinType;
use anyhow::Result;
use cosmos_sdk_proto::cosmos::bank::v1beta1::{query_client::QueryClient, QueryBalanceRequest};
use http::uri::Uri;
use std::str::FromStr;
use tendermint_rpc::Url;
use web3::types::Address;

impl CoinType {
    pub async fn get_balance(
        &self,
        address: String,
        denom: String,
        grcp_addr: Option<Url>,
        evm_addr: Option<Url>,
    ) -> Result<String> {
        match self {
            CoinType::COSMOS => {
                get_cosmos_balance(address, denom, grcp_addr.unwrap().to_string()).await
            }
            CoinType::EVM => get_evm_balance(address, evm_addr.unwrap().to_string()).await,
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
    let request = QueryBalanceRequest { address, denom: denom.clone() };
    Ok(query_client
        .balance(request)
        .await?
        .into_inner()
        .balance
        .map(|coin| coin.amount)
        .ok_or_else(|| crate::error::Error::get_cosmos_balance(denom))?
    )
}

pub async fn get_evm_balance(address: String, evm_addr: String) -> Result<String> {
    let transport = web3::transports::Http::new(&evm_addr)?;
    let web3 = web3::Web3::new(transport);
    let account = Address::from_str(&address)?;
    let balance = web3.eth().balance(account, None).await?;
    Ok(balance.as_u128().to_string())
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
}
