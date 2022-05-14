use binance::api::{Binance, Futures};
use binance::futures::general::FuturesGeneral;
use binance::market::Market;
use clap::Parser;
use tokio::runtime::Runtime;

#[derive(clap::Parser, Debug)]
pub struct ArbitrageCommand {
    #[clap(long)]
    pub symbols: Vec<String>,

    #[clap(long, default_value = "2.0")]
    pub delta: f64,

    #[clap(long, default_value = "")]
    pub token: String,

    #[clap(long)]
    pub channel: u64,
}

pub struct Arbitrage {
    cmd: ArbitrageCommand,
    rt: Runtime,
}

impl Arbitrage {
    pub fn new(cmd: ArbitrageCommand) -> Self {
        let rt = tokio::runtime::Runtime::new().unwrap();
        Self { rt, cmd }
    }

    pub fn run(self) {
        let market: Market = Binance::new(None, None);

        for s in self.cmd.symbols.iter() {
            let sp = market.get_price(s).unwrap();

            let fut = binance::futures::market::FuturesMarket::new(None, None);
            let mp = fut.get_price(s).unwrap();

            let delta = (sp.price - mp.price).abs() * 100_f64 / sp.price;
            let msg = format!(
                "币安期货套利 {} delta={}% spot={:?} margin={:?}",
                s, delta, sp.price, mp.price
            );
            if delta >= self.cmd.delta {
                println!("{}", msg);
                self.send_message(msg);
            }
        }
    }

    pub fn send_message(&self, msg: String) {
        self.rt.block_on(async move {
            crate::send_messages(&self.cmd.token, self.cmd.channel, msg).await;
        });
    }
}
