use binance::model::{Asks, Bids, OrderBook};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Div, DivAssign, Sub};
use std::str::FromStr;
use yaml_rust::{YamlEmitter, YamlLoader};

#[derive(Deserialize)]
pub struct OrderbookConfig {
    pub symbols: Vec<OrderBookSymbol>,
}

impl From<Vec<String>> for OrderbookConfig {
    fn from(symbols: Vec<String>) -> Self {
        let symbols = symbols
            .iter()
            .map(|s| OrderBookSymbol::from_str(s).unwrap())
            .collect::<Vec<_>>();
        Self { symbols }
    }
}

impl FromStr for OrderbookConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let f = std::fs::File::open(s).map_err(|e| anyhow::anyhow!("{}", e))?;
        let ob = serde_yaml::from_reader::<_, Self>(f).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        Ok(ob)
    }
}

#[derive(Deserialize)]
pub struct OrderBookSymbol {
    pub symbol: String,
    pub aggregate: f64,
}

impl FromStr for OrderBookSymbol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t: Vec<_> = s.split('=').collect();
        if t.len() != 2 {
            return Err(anyhow::anyhow!("format: BTC_USDT=100.0"));
        }

        let symbol = t[0].to_ascii_uppercase();
        let aggregate: f64 = t[1].parse::<f64>().map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(Self { symbol, aggregate })
    }
}

#[derive(Debug)]
pub struct F64(f64);

impl std::ops::Deref for F64 {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0 as u64);
        state.finish();
    }
}

impl PartialEq<Self> for F64 {
    fn eq(&self, other: &Self) -> bool {
        return self.0.sub(other.0) < f64::EPSILON;
    }
}

impl Eq for F64 {}

#[derive(Debug, Serialize, Deserialize)]
pub struct PilotBids {
    pub price: f64,
    pub volume: f64,
}

impl From<Bids> for PilotBids {
    fn from(bids: Bids) -> Self {
        Self {
            price: bids.price,
            volume: bids.qty * bids.price,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PilotAsks {
    pub price: f64,
    pub volume: f64,
}

impl From<Asks> for PilotAsks {
    fn from(asks: Asks) -> Self {
        Self {
            price: asks.price,
            volume: asks.qty * asks.price,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PilotOrderBook {
    pub symbol: String,
    pub created: i64,
    pub asks: Vec<PilotAsks>,
    pub bids: Vec<PilotBids>,
}

impl From<(String, f64, OrderBook)> for PilotOrderBook {
    fn from(x: (String, f64, OrderBook)) -> Self {
        let symbol = x.0;
        let delta = x.1;
        let ob = x.2;
        let created = chrono::Local::now().timestamp();
        let bids = ob
            .bids
            .into_iter()
            .map(|b| PilotBids::from(b))
            .collect::<Vec<_>>();

        let mut bids = bids
            .into_iter()
            .map(|b| (F64(f64::trunc(b.price / delta) * delta), b.volume))
            .into_group_map()
            .into_iter()
            .map(|(key, group)| PilotBids {
                price: key.0,
                volume: group.iter().sum(),
            })
            .collect::<Vec<_>>();

        let asks = ob
            .asks
            .into_iter()
            .map(|a| PilotAsks::from(a))
            .collect::<Vec<_>>();

        let mut asks = asks
            .into_iter()
            .map(|b| (F64(f64::trunc(b.price / delta) * delta), b.volume))
            .into_group_map()
            .into_iter()
            .map(|(key, group)| PilotAsks {
                price: key.0,
                volume: group.iter().sum(),
            })
            .collect::<Vec<_>>();

        bids.sort_unstable_by(|a, b| {
            b.price
                .partial_cmp(&a.price)
                .unwrap_or_else(|| Ordering::Less)
        });
        asks.sort_unstable_by(|a, b| {
            b.price
                .partial_cmp(&a.price)
                .unwrap_or_else(|| Ordering::Less)
        });

        Self {
            asks,
            bids,
            created,
            symbol,
        }
    }
}
