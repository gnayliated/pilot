use crate::helper;
use crate::helper::LCOption;
use crate::orderbook::{OrderbookConfig, PilotOrderBook};
use binance::api::Binance;
use binance::market::Market;
use std::str::FromStr;

#[derive(clap::Parser, Debug)]
pub struct OrderbookCommand {
    #[clap(long)]
    pub symbols: Vec<String>,

    #[clap(long, default_value = "")]
    pub lc_id: String,

    #[clap(long, default_value = "")]
    pub lc_baseuri: String,

    #[clap(long, default_value = "")]
    pub lc_key: String,
}

pub struct OrderbookFetcher {
    cmd: OrderbookCommand,
}

impl OrderbookFetcher {
    pub fn new(cmd: OrderbookCommand) -> Self {
        Self { cmd }
    }

    pub fn run(self) {
        self.fetch_orderbook_and_push();
    }

    pub fn fetch_orderbook_and_push(&self) {
        let cfg = OrderbookConfig::from(self.cmd.symbols.clone());

        cfg.symbols.iter().for_each(|s| {
            let market: Market = Binance::new(None, None);
            let orderbook = PilotOrderBook::from((
                s.symbol.clone(),
                s.aggregate,
                market.get_custom_depth(&s.symbol, 5000).unwrap(),
            ));

            self.push_leancloud(&s.symbol, orderbook);
        })
    }

    pub fn push_leancloud(&self, symbol: &str, ob: PilotOrderBook) {
        let (_, today) = helper::load_yes_and_today();
        let start = today.format("%Y%m%d").to_string();

        let opt = LCOption {
            class_uri: format!("/1.1/classes/ob_{}_{}", symbol.to_ascii_lowercase(), start),
            uri: format!("{}/1.1/batch", self.cmd.lc_baseuri),
            id: self.cmd.lc_id.clone(),
            key: self.cmd.lc_key.clone(),
            from: "binance".to_string(),
        };

        let status = helper::post_orderbook_leancloud(opt.clone(), ob).unwrap();

        log::info!("POST opt={:?} status={}", opt, status);
    }

    // pub fn push_gists(&self, ob: PilotOrderBook) {
    //     let gist_token = self.cmd.gist_token.clone();
    //     tokio::runtime::Runtime::new().unwrap().block_on(async {
    //         let github = Client::new(
    //             String::from("pilot-pipeline"),
    //             Credentials::Token(gist_token),
    //         )
    //         .unwrap();
    //
    //         let now = chrono::Utc::now().format("%F-%H").to_string();
    //         let description = format!("{}-{}", ob.symbol, now);
    //         let g = gists::Gists::new(github);
    //         let rv = g.list_all(None).await.unwrap();
    //         let id = match rv.iter().find(|g| g.description == description) {
    //             Some(g) => g.id.clone(),
    //             None => {
    //                 let req = GistsCreateRequest {
    //                     description: description.clone(),
    //                     r#public: Some(PublicOneOf::Bool(true)),
    //                     files: maplit::hashmap! {
    //                         description.clone() => FilesAdditionalPropertiesData {
    //                             content: description.clone(),
    //                         }
    //                     },
    //                 };
    //                 let s = g.create(&req).await.unwrap();
    //                 s.id
    //             }
    //         };
    //         let req = PullsUpdateReviewRequest {
    //             body: serde_json::to_string(&ob).unwrap(),
    //         };
    //         let comment = g.create_comment(&id, &req).await.unwrap();
    //         log::debug!("comment {:?}", comment)
    //     });
    // }
    //
    // pub fn fetch_and_push_prometheus(&self) {
    //     let market: Market = Binance::new(
    //         Some(binance_key.to_string()),
    //         Some(binance_secret.to_string()),
    //     );
    //
    //     let stats = market.get_all_24h_price_stats().unwrap();
    //
    //     log::info!("loading 24h price stats {}", stats.len());
    //     stats.iter().for_each(|stat| {
    //         log::debug!(
    //             "{}: last={} count={} price_change_percent={}",
    //             stat.symbol,
    //             stat.last_price,
    //             stat.count,
    //             stat.price_change_percent,
    //         );
    //     });
    //
    //     let wq = helper::build_remote_from_stats(&stats);
    //     let status = crate::helper::write_remote_prom(&self.cmd, wq).unwrap();
    //
    //     log::info!("POST {} status={}", self.cmd.endpoint_uri, status);
    // }
    //
    // pub fn fetch_and_push_symbols_price(&self) {
    //     let symbols = self.cmd.symbols.deref();
    //     let mut prices = Vec::new();
    //     for i in 0..self.cmd.retry {
    //         prices = match self.get_average_prices(&symbols) {
    //             Ok(p) => p,
    //             Err(err) => {
    //                 log::error!("get average prices error: {}", err);
    //                 std::thread::sleep(std::time::Duration::from_secs(15));
    //                 continue;
    //             }
    //         }
    //     }
    //
    //     log::debug!("{:?} = {:?}", symbols, prices);
    //
    //     let series = symbols
    //         .iter()
    //         .zip(prices)
    //         .map(|(x, y)| (x.to_string(), y))
    //         .collect::<Vec<_>>();
    //
    //     let wq = helper::build_remote_symbols("btc_price", series);
    //     let status = crate::helper::write_remote_prom(&self.cmd, wq).unwrap();
    //
    //     log::info!("POST {} status={}", self.cmd.endpoint_uri, status);
    // }
    //
    // fn get_average_prices(&self, symbols: &[String]) -> anyhow::Result<Vec<f64>> {
    //     let (tx, rx) = channel();
    //
    //     symbols.iter().for_each(|symbol| {
    //         let symbol = symbol.to_string();
    //         let tx = tx.clone();
    //         self.pool.execute(move || {
    //             let price = Self::get_average_price(&symbol);
    //             let rv = tx.send(price);
    //             if let Err(e) = rv {
    //                 log::error!("channel {}", e);
    //             }
    //         });
    //     });
    //
    //     let prices = rx
    //         .iter()
    //         .take(symbols.len())
    //         .collect::<anyhow::Result<Vec<f64>>>()?;
    //
    //     Ok(prices)
    // }
    //
    // fn get_average_price(symbol: &str) -> anyhow::Result<f64> {
    //     let market: Market = Binance::new(
    //         Some(binance_key.to_string()),
    //         Some(binance_secret.to_string()),
    //     );
    //     let p = market
    //         .get_average_price(symbol)
    //         .map_err(|e| anyhow::anyhow!("{} {:?}", symbol, e))?;
    //     Ok(p.price)
    // }
}
