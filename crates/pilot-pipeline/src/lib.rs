mod helper;

use crate::helper::Symbols;
use binance::api::*;
use binance::market::*;
use clap::Parser;
use helper::parse_lines;
use std::ops::Deref;
use std::sync::mpsc::channel;
use threadpool::ThreadPool;

const binance_key: &str = "";
const binance_secret: &str = "";

#[derive(clap::Parser, Debug)]
pub struct Command {
    #[clap(long)]
    pub once: bool,

    #[clap(long, default_value_t = 3)]
    pub retry: u8,

    #[clap(long)]
    pub endpoint_auth_user: String,

    #[clap(long)]
    pub endpoint_auth_pass: String,

    #[clap(long)]
    pub endpoint_uri: String,

    #[clap(long, default_value_t = 1)]
    pub workers: u8,

    #[clap(long, parse(try_from_str = parse_lines), default_value = "")]
    pub symbols: Symbols,
}

pub struct Pipeline {
    cmd: Command,
    pool: ThreadPool,
}

impl Pipeline {
    pub fn new(cmd: Command) -> Self {
        let pool = ThreadPool::new(cmd.workers as usize);

        Self { pool, cmd }
    }

    pub fn run(self) {
        if self.cmd.once {
            self.fetch_and_push_prometheus();
            return;
        }
        todo!()
    }

    pub fn fetch_and_push_prometheus(&self) {
        let market: Market = Binance::new(
            Some(binance_key.to_string()),
            Some(binance_secret.to_string()),
        );

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

    pub fn fetch_and_push_symbols_price(&self) {
        let symbols = self.cmd.symbols.deref();
        let mut prices = Vec::new();
        for i in 0..self.cmd.retry {
            prices = match self.get_average_prices(&symbols) {
                Ok(p) => p,
                Err(err) => {
                    log::error!("get average prices error: {}", err);
                    std::thread::sleep(std::time::Duration::from_secs(15));
                    continue;
                }
            }
        }

        log::debug!("{:?} = {:?}", symbols, prices);

        let series = symbols
            .iter()
            .zip(prices)
            .map(|(x, y)| (x.to_string(), y))
            .collect::<Vec<_>>();

        let wq = helper::build_remote_symbols("btc_price", series);
        let status = crate::helper::write_remote_prom(&self.cmd, wq).unwrap();

        log::info!("POST {} status={}", self.cmd.endpoint_uri, status);
    }

    fn get_average_prices(&self, symbols: &[String]) -> anyhow::Result<Vec<f64>> {
        let (tx, rx) = channel();

        symbols.iter().for_each(|symbol| {
            let symbol = symbol.to_string();
            let tx = tx.clone();
            self.pool.execute(move || {
                let price = Self::get_average_price(&symbol);
                let rv = tx.send(price);
                if let Err(e) = rv {
                    log::error!("channel {}", e);
                }
            });
        });

        let prices = rx
            .iter()
            .take(symbols.len())
            .collect::<anyhow::Result<Vec<f64>>>()?;

        Ok(prices)
    }

    fn get_average_price(symbol: &str) -> anyhow::Result<f64> {
        let market: Market = Binance::new(
            Some(binance_key.to_string()),
            Some(binance_secret.to_string()),
        );
        let p = market
            .get_average_price(symbol)
            .map_err(|e| anyhow::anyhow!("{} {:?}", symbol, e))?;
        Ok(p.price)
    }
}
