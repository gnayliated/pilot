use crate::helper;
use crate::helper::parse_lines;
use binance::api::Binance;
use binance::market::Market;
use binance::websockets::WebsocketEvent::OrderBook;
use clap::Parser;
use log::debug;
use octorust::types::{
    FilesAdditionalPropertiesData, GistsCreateRequest, PublicOneOf, PullsUpdateReviewRequest,
};
use octorust::{auth::Credentials, gists, Client};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::mpsc::channel;
use threadpool::ThreadPool;

#[derive(clap::Parser, Debug)]
pub struct PriceCommand {
    #[clap(long, default_value = "")]
    pub endpoint_auth_user: String,

    #[clap(long, default_value = "")]
    pub endpoint_auth_pass: String,

    #[clap(long, default_value = "")]
    pub endpoint_uri: String,
}

pub struct PriceFetcher {
    cmd: PriceCommand,
}

impl PriceFetcher {
    pub fn new(cmd: PriceCommand) -> Self {
        Self { cmd }
    }

    pub fn run(self) {
        self.fetch_and_push_prometheus();
    }

    pub fn fetch_and_push_prometheus(&self) {
        let market: Market = Binance::new(Some("".to_string()), Some("".to_string()));

        let stats = market.get_all_24h_price_stats().unwrap();

        log::info!("loading 24h price stats {}", stats.len());
        stats.iter().for_each(|stat| {
            log::debug!(
                "{}: last={} count={} price_change_percent={}",
                stat.symbol,
                stat.last_price,
                stat.count,
                stat.price_change_percent,
            );
        });

        let wq = helper::build_remote_from_stats(&stats);
        let status = crate::helper::write_remote_prom(&self.cmd, wq).unwrap();

        log::info!("POST {} status={}", self.cmd.endpoint_uri, status);
    }
}
