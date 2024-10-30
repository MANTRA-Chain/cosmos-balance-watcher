# Cosmos Balance Watcher 🧑🏻‍🏭

Query native token balance for cosmos-sdk and evm chains, and expose account balance status as prometheus metrics.
One can send alert based on prometheus alerting rules.

## Build

```bash
make all
```

## Prepare config file

example
```toml
[prometheus]
host = '127.0.0.1'
port = 9090

[[chains]]
id = 'chain_A'
grpc_addr = 'http://127.0.0.1:9090'
[[chains.addresses]]
address = 'mantra1q040rm026jmpfmxdsj6q9phm9tdceepnsau6me'
disable_balance = true
role = 'personal'
refresh = '300s'
[[chains.addresses.coins]]
denom = 'uom'
min_balance = '200000000000'
[[chains.addresses.coins]]
denom = 'ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239'
min_balance = '200000000000'
[[chains.addresses]]
address = 'mantra1q040rm026jmpfmxdsj6q9phm9tdceepnsau6m2'
disable_balance = false
role = 'relayer'
[[chains.addresses.coins]]
denom = 'uom'
min_balance = '200000000000'

[[chains]]
id = 'chain_B'
grpc_addr = 'http://127.0.0.1:9090'
[[chains.addresses]]
address = 'mantra1q040rm026jmpfmxdsj6q9phm9tdceepnsau6me'
disable_balance = false
role = 'faucet'
refresh = '300s'
[[chains.addresses.coins]]
denom = 'uom'
min_balance = '1000000000000000000000000000000000'

[[chains]]
id = '1'
evm_addr = 'https://eth.llamarpc.com'
[[chains.addresses]]
address = '0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B'
decimal_place = 18
role = 'vitalik'
refresh = '300s'
balance_url = 'https://etherscan.io/address/0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B'
[[chains.addresses.coins]]
coin_type = "EVM"
denom = 'wei'
display_denom = 'ETH'
min_balance = '10000000000000000000'
```

## Run

```bash
./target/debug/balance-watcher start -c YOUR_CONFIG_PATH
```

## Show prometheus metrics
```bash
$ curl http://127.0.0.1:9090/metrics

# HELP account_balance account balance
# TYPE account_balance gauge
account_balance{address="0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",balance_url="https://etherscan.io/address/0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",chain_id="1",denom="ETH",min_balance="10",role="vitalik"} 37
account_balance{address="mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",chain_id="mantra-dukong-1",denom="OM",min_balance="1000000",role="test2"} 9
account_balance{address="mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",chain_id="mantra-dukong-1",denom="OM",min_balance="200000",role="test1"} 11
# HELP account_query_status Account Query Status show the account balance query is successful or not. 0: can access, 1: cannot access
# TYPE account_query_status gauge
account_query_status{address="0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",balance_url="https://etherscan.io/address/0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",chain_id="1",denom="ETH",min_balance="10",role="vitalik"} 0
account_query_status{address="mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",chain_id="mantra-dukong-1",denom="OM",min_balance="1000000",role="test2"} 0
account_query_status{address="mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",chain_id="mantra-dukong-1",denom="OM",min_balance="200000",role="test1"} 0
# HELP account_status Account Status. 0: > min_balance, 1: <= min_balance
# TYPE account_status gauge
account_status{address="0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",balance_url="https://etherscan.io/address/0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B",chain_id="1",denom="ETH",min_balance="10",role="vitalik"} 0
account_status{address="mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1ea4hlqfskjvn0ldenw8gv7jjdzrljcchm9vhhu",chain_id="mantra-dukong-1",denom="OM",min_balance="1000000",role="test2"} 1
account_status{address="mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",balance_url="https://www.mintscan.io/mantra-testnet/address/mantra1q8mgs55hfgkm7d5rret439997x87s2ekwcxlv0",chain_id="mantra-dukong-1",denom="OM",min_balance="200000",role="test1"} 1
```
