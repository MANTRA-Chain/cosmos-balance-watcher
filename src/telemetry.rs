use lazy_static::lazy_static;
use log::error;
use prometheus::{IntGaugeVec, Opts, Registry};
use warp::{Rejection, Reply};

lazy_static! {
    pub static ref ACCOUNT_BALANCE_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_balance", "account balance"),
        &["chain_id", "address", "denom", "role", "balance_url"]
    )
    .expect("metric can be created");
    pub static ref ACCOUNT_STATUS_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_status", "Account Status. 0: > min_balance, 1: <= min_balance"),
        &["chain_id", "address", "denom", "min_balance", "role", "balance_url"]
    )
    .expect("metric can be created");
    pub static ref ACCOUNT_QUERY_STATUS_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_query_status", "Account Query Status show the account balance query is successful or not. 0: can access, 1: cannot access"),
        &["chain_id", "address", "role", "balance_url", "query_endpoint_url"]
    )
    .expect("metric can be created");

    pub static ref REGISTRY: Registry = Registry::new();
}

/// A setter for ACCOUNT_BALANCE_COLLECTOR, make sure all the labels are set and types are correct
pub fn account_balance_setter(
    chain_id: &str,
    address: &str,
    denom: &str,
    role: &str,
    balance_url: &str,
    balance: i64,
) {
    ACCOUNT_BALANCE_COLLECTOR
        .with_label_values(&[chain_id, address, denom, role, balance_url])
        .set(balance);
}

/// A setter for ACCOUNT_STATUS_COLLECTOR, make sure all the labels are set and types are correct
pub fn account_status_setter(
    chain_id: &str,
    address: &str,
    denom: &str,
    min_balance: &str,
    role: &str,
    balance_url: &str,
    status: i64,
) {
    ACCOUNT_STATUS_COLLECTOR
        .with_label_values(&[chain_id, address, denom, min_balance, role, balance_url])
        .set(status);
}

/// A setter for ACCOUNT_QUERY_STATUS_COLLECTOR, make sure all the labels are set and types are correct
pub fn account_query_status_setter(
    chain_id: &str,
    address: &str,
    role: &str,
    balance_url: &str,
    query_endpoint_url: &str,
    status: i64,
) {
    ACCOUNT_QUERY_STATUS_COLLECTOR
        .with_label_values(&[chain_id, address, role, balance_url, query_endpoint_url])
        .set(status);
}

pub fn register_custom_metrics() {
    REGISTRY
        .register(Box::new(ACCOUNT_BALANCE_COLLECTOR.clone()))
        .expect("collector can be registered");
    REGISTRY
        .register(Box::new(ACCOUNT_STATUS_COLLECTOR.clone()))
        .expect("collector can be registered");
    REGISTRY
        .register(Box::new(ACCOUNT_QUERY_STATUS_COLLECTOR.clone()))
        .expect("collector can be registered");
}

pub async fn metrics_handler() -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
        error!("could not encode custom metrics: {:?}", e);
    };
    let mut res = String::from_utf8(buffer.clone()).unwrap_or_else(|e| {
        error!("custom metrics could not be from_utf8'd: {}", e);
        String::default()
    });
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        error!("could not encode prometheus metrics: {:?}", e);
    };
    let res_custom = String::from_utf8(buffer.clone()).unwrap_or_else(|e| {
        error!("prometheus metrics could not be from_utf8'd: {}", e);
        String::default()
    });
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}
