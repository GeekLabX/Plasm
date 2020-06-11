//! Web-server helper for Lockdrop runtime module.

use codec::Decode;
use pallet_plasm_lockdrop::LockCheck;
use web3::futures::Future;
use tide::{http::StatusCode, Response};

mod btc_utils;
mod eth_utils;

const COINGECKO_BTC_API: &'static str = "https://api.coingecko.com/api/v3/coins/bitcoin";
const COINGECKO_ETH_API: &'static str = "https://api.coingecko.com/api/v3/coins/ethereum";

const BLOCKCYPHER_BTC_API: &'static str = "https://api.blockcypher.com/v1/btc/test3/txs";
const INFURA_ETH_API: &'static str = "https://ropsten.infura.io/v3/e673f1634f174971890bf13130751704";

const LOCKDROP_ETH_CONTRACT: &'static str = "EEd84A89675342fB04faFE06F7BB176fE35Cb168";
const SAFE_ETH_CONFIRMATIONS: u64 = 10;

pub async fn start() {
    let mut app = tide::new();

    app.at("/btc/ticker").get(|_| async {
        let ticker: serde_json::Value = reqwest::blocking::get(COINGECKO_BTC_API)?.json()?;
        Ok(ticker["market_data"]["current_price"]["usd"].to_string())
    });

    app.at("/eth/ticker").get(|_| async {
        let ticker: serde_json::Value = reqwest::blocking::get(COINGECKO_ETH_API)?.json()?;
        Ok(ticker["market_data"]["current_price"]["usd"].to_string())
    });

    app.at("/btc/lock")
        .post(|mut req: tide::Request<()>| async move {
            let body = req.body_bytes().await?;
            let lock = LockCheck::decode(&mut &body[..])?;
            log::debug!(
                target: "lockdrop-oracle",
                "BTC lock check request: {:#?}", lock
            );

            let uri = format!("{}/{}", BLOCKCYPHER_BTC_API, hex::encode(lock.tx_hash));
            let tx: serde_json::Value = reqwest::blocking::get(uri.as_str())?.json()?;
            log::debug!(
                target: "lockdrop-oracle",
                "BTC tx at {}: {}", lock.tx_hash, tx.to_string()
            );

            let tx_sender = tx["inputs"][0]["addresses"].to_string();
            let tx_value = tx["outputs"][0]["value"].as_u64().unwrap_or(0) as u128;
            let tx_script = hex::decode(tx["outputs"][0]["script"].to_string())?;
            let tx_confirmations = tx["confirmations"].as_u64().unwrap_or(0);

            // check transaction confirmations
            if tx_confirmations < 8 {
                log::debug!(target: "lockdrop-oracle", "transaction isn't confirmed yet");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            // check transaction value
            if tx_value != lock.value {
                log::debug!(target: "lockdrop-oracle", "lock value mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            let lock_sender = btc_utils::to_address(&lock.public_key, btc_utils::BTC_TESTNET);
            log::debug!(
                target: "lockdrop-oracle",
                "BTC address for public key {}: {}",
                lock.public_key,
                lock_sender,
            );
            // check transaction sender address
            if tx_sender != lock_sender {
                log::debug!(target: "lockdrop-oracle", "sender address mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            // assembly bitcoin script for given params
            let lock_script = btc_utils::lock_script(&lock.public_key, lock.duration);
            log::debug!(
                target: "lockdrop-oracle",
                "BTC lock script for public key ({}) and duration ({}): {}",
                lock.public_key,
                lock.duration,
                hex::encode(lock_script.clone()),
            );
            let script = btc_utils::p2sh(&btc_utils::script_hash(&lock_script[..]));
            log::debug!(
                target: "lockdrop-oracle",
                "P2SH for script {}: {}",
                hex::encode(lock_script),
                hex::encode(&script),
            );
            // check script code
            if tx_script != script {
                log::debug!(target: "lockdrop-oracle", "lock script mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            Ok(Response::new(StatusCode::Ok))
        });

    app.at("/eth/lock")
        .post(|mut req: tide::Request<()>| async move {
            let body = req.body_bytes().await?;
            let lock = LockCheck::decode(&mut &body[..])?;
            log::debug!(
                target: "lockdrop-oracle",
                "ETH lock check request: {:#?}", lock
            );

            let (_eloop, transport) = web3::transports::Http::new(INFURA_ETH_API).unwrap();
            let web3 = web3::Web3::new(transport);
            let block_number = web3.eth().block_number().wait()?;
            let tx = web3
                .eth()
                .transaction(web3::types::TransactionId::Hash(lock.tx_hash))
                .wait()?
                .unwrap();
            log::debug!(
                target: "lockdrop-oracle",
                "Ethereum transaction at {}: {:#?}", lock.tx_hash, tx
            );

            // check transaction value
            if tx.value != lock.value.into() {
                log::debug!(target: "lockdrop-oracle", "lock value mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            let tx_confirmations = block_number - tx.block_number.unwrap_or(Default::default());
            log::debug!(
                target: "lockdrop-oracle",
                "Transaction confirmations: {}", tx_confirmations
            );
            if tx_confirmations < SAFE_ETH_CONFIRMATIONS.into() {
                log::debug!(target: "lockdrop-oracle", "transaction isn't confirmed yet");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            let sender = eth_utils::to_address(lock.public_key.as_ref()).unwrap_or(Default::default());
            log::debug!(
                target: "lockdrop-oracle",
                "ETH address for public key {}: {}",
                lock.public_key, sender
            );
            // check sender address
            if tx.from != sender {
                log::debug!(target: "lockdrop-oracle", "sender address mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            // check that destination is lockdrop smart contract
            if tx.to != Some(LOCKDROP_ETH_CONTRACT.parse()?) {
                log::debug!(target: "lockdrop-oracle", "contract address mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            // check smart contract method input
            if !eth_utils::lock_method_check(tx.input.0.as_ref(), lock.duration) {
                log::debug!(target: "lockdrop-oracle", "lock method mismatch");
                return Ok(Response::new(StatusCode::BadRequest))
            }

            Ok(Response::new(StatusCode::Ok))
        });

    app.listen("127.0.0.1:34347")
        .await
        .map_err(|e| log::error!("oracle web-server error: {}", e))
        .unwrap_or(());
}