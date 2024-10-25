use log::error;
use prometheus::{IntGaugeVec, Opts, Registry};
use warp::{Rejection, Reply};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref ACCOUNT_BALANCE_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_balance", "account balance"),
        &["chain_id", "address", "denom", "min_balance", "role", "balance_url"]
    )
    .expect("metric can be created");
    pub static ref ACCOUNT_STATUS_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_status", "Account Status. 0: > min_balance, 1: < min_balance"),
        &["chain_id", "address", "denom", "min_balance", "role", "balance_url"]
    )
    .expect("metric can be created");
    pub static ref ACCOUNT_QUERY_STATUS_COLLECTOR: IntGaugeVec = IntGaugeVec::new(
        Opts::new("account_query_status", "Account Query Status show the account balance query is successful or not. 0: can access, 1: cannot access"),
        &["chain_id", "address", "denom", "min_balance", "role", "balance_url"]
    )
    .expect("metric can be created");

    pub static ref REGISTRY: Registry = Registry::new();
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
    let mut res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            error!("custom metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        error!("could not encode prometheus metrics: {:?}", e);
    };
    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            error!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();

    res.push_str(&res_custom);
    Ok(res)
}
