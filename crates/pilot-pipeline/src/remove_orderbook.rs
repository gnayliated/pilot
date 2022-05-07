use crate::helper;
use crate::helper::LCOption;
use crate::orderbook::{OrderbookConfig, PilotOrderBook};
use binance::api::Binance;
use binance::market::Market;
use serde::Deserialize;
use std::str::FromStr;
use std::time::Duration;

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

    #[clap(
        long,
        default_value = "https://us-w1-console-api.leancloud.app/1.1/signin"
    )]
    pub lc_login_uri: String,

    #[clap(
        long,
        default_value = "https://us-w1-console-api.leancloud.app/1.1/data/prfqn7TYyAT5g1xct4B98wgU-MdYXbMMI/classes"
    )]
    pub lc_delete_uri: String,

    #[clap(
        long,
        default_value = "https://us-w1-console-api.leancloud.app/1.1/xsrf-token"
    )]
    pub lc_xsrf_uri: String,

    #[clap(long, default_value = "email")]
    pub lc_email: String,

    #[clap(long, default_value = "pass")]
    pub lc_pass: String,
}

pub struct OrderbookRemover {
    cmd: OrderbookCommand,
}

#[derive(Deserialize, Debug)]
pub struct XSRFData {
    #[serde(rename = "xsrf-token")]
    xsrf: String,
    ttl: u64,
}

impl OrderbookRemover {
    pub fn new(cmd: OrderbookCommand) -> Self {
        Self { cmd }
    }

    pub fn run(self) {
        self.login_and_remove();
    }

    fn login_and_remove(&self) {
        let client = self.login().unwrap();
        // {"xsrf-token":"a7a57ab6458c89377b824a934718f1868ffbadae83b0acee3485b19facbd4a96","ttl":604800}
        let x: XSRFData = client
            .get(&self.cmd.lc_xsrf_uri)
            .send()
            .unwrap()
            .json()
            .unwrap();
        log::info!("xsrf={:?}", x);

        let now = chrono::Utc::now();
        let today = now.date();
        let day2ago = today - chrono::Duration::days(2);
        let day2 = day2ago.format("%Y%m%d");

        for symbol in self.cmd.symbols.iter() {
            let delete_uri = format!(
                "{}/ob_{}_{}",
                self.cmd.lc_delete_uri,
                symbol.to_lowercase(),
                day2
            );
            let res = client
                .delete(delete_uri)
                .header("x-xsrf-token", &x.xsrf)
                .send();
            log::info!("res {:?}", res);
        }
    }

    fn login(&self) -> anyhow::Result<reqwest::blocking::Client> {
        let client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();
        let res = client
            .post(&self.cmd.lc_login_uri)
            .json(&serde_json::json!({
                "email":	self.cmd.lc_email,
                "password": self.cmd.lc_pass,
            }))
            .send()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        if res.status().as_u16() >= 300 {
            return Err(anyhow::anyhow!("bad status {}", res.status()));
        }
        Ok(client)
    }
}
