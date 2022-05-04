use crate::helper;
use crate::helper::{LCOption, LCOrderBookResponse, OrderbookRow};
use crate::orderbook::{OrderbookConfig, PilotOrderBook};
use binance::api::Binance;
use binance::market::Market;
use chrono::Duration;
use parquet::basic::Compression;
use parquet::column::writer::ColumnWriter;
use parquet::data_type::ByteArray;
use parquet::file::properties::WriterProperties;
use parquet::file::writer::{FileWriter, SerializedFileWriter};
use parquet::schema::parser::parse_message_type;
use std::array::from_mut;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

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

    #[clap(long, default_value = "")]
    pub gh_token: String,

    #[clap(long, default_value = "gnayliated")]
    pub owner: String,

    #[clap(long, default_value = "orderbook-snapshot")]
    pub repo: String,

    #[clap(long, default_value = "main")]
    pub branch: String,

    #[clap(long, default_value = "gnayliated")]
    pub name: String,

    #[clap(long, default_value = "detailyang@icloud.com")]
    pub email: String,
}

pub struct OrderbookLoader {
    cmd: OrderbookCommand,
}

impl OrderbookLoader {
    pub fn new(cmd: OrderbookCommand) -> Self {
        Self { cmd }
    }

    pub fn run(self) {
        let cfg = OrderbookConfig::from(self.cmd.symbols.clone());
        let (yesterday, _) = helper::load_yes_and_today();
        let start = yesterday.format("%Y%m%d").to_string();
        let mut paths = vec![];

        cfg.symbols.iter().for_each(|s| {
            let res = self.load_orderbook_leancloud(&s.symbol, &start);
            let target = format!("./target/{}-{}.parquet", s.symbol.to_lowercase(), start);
            let path = format!(
                "{}/{}-{}.parquet",
                s.symbol.to_lowercase(),
                s.symbol.to_lowercase(),
                start
            );
            self.save_order_book_parquet(&target, res);
            paths.push((target, path));
        });

        for (target, path) in paths.iter() {
            let content = fs::read(target).expect("Unable to read file");
            self.upload_file_to_repo(path, content);
        }
    }

    pub fn upload_file_to_repo(&self, path: &str, content: impl AsRef<[u8]>) {
        let owner = &self.cmd.owner;
        let repo = &self.cmd.repo;
        let message = format!("commit {}", path);
        let branch = &self.cmd.branch;
        let name = &self.cmd.name;
        let email = &self.cmd.email;
        let token = self.cmd.gh_token.to_string();

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let update = helper::create_file_to_repo(
                    &token, owner, repo, path, message, content, branch, name, email,
                )
                .await
                .unwrap();
                log::info!("update {:?}", update)
            });
    }

    pub fn delete_orderbook_leancloud(&self, symbol: &str, start: i64, end: i64) {
        let opt = LCOption {
            class_uri: format!("/1.1/classes/ob_{}_{}", symbol.to_ascii_lowercase(), start),
            uri: format!(
                "{}/1.1/classes/ob_{}_{}",
                self.cmd.lc_baseuri,
                symbol.to_ascii_lowercase(),
                start
            ),
            id: self.cmd.lc_id.clone(),
            key: self.cmd.lc_key.clone(),
            from: "binance".to_string(),
        };

        let res = helper::delete_orderbook_leancloud(opt.clone(), start, end).unwrap();

        log::info!("DELETE opt={:?} res={:?}", opt, res);
    }

    pub fn load_orderbook_leancloud(&self, symbol: &str, start: &str) -> LCOrderBookResponse {
        let opt = LCOption {
            class_uri: format!("/1.1/classes/ob_{}_{}", symbol.to_ascii_lowercase(), start),
            uri: format!(
                "{}/1.1/classes/ob_{}_{}",
                self.cmd.lc_baseuri,
                symbol.to_ascii_lowercase(),
                start
            ),
            id: self.cmd.lc_id.clone(),
            key: self.cmd.lc_key.clone(),
            from: "binance".to_string(),
        };

        let res = helper::load_orderbook_leancloud(opt.clone()).unwrap();

        log::info!("POST opt={:?} res={:?}", opt, res);
        res
    }

    pub fn save_order_book_parquet(&self, path: &str, ob: LCOrderBookResponse) {
        let path = Path::new(path);

        let mut data: Vec<OrderbookRow> = vec![];
        ob.results.iter().for_each(|x| {
            x.bids.iter().for_each(|y| {
                data.push(OrderbookRow {
                    timestamp: x.created,
                    price: y.price,
                    volume: y.volume,
                    from: x.from.clone(),
                });
            });

            x.asks.iter().for_each(|y| {
                data.push(OrderbookRow {
                    timestamp: x.created,
                    price: y.price,
                    volume: y.volume,
                    from: x.from.clone(),
                });
            });
        });
        let ts_cols = data.iter().map(|x| x.timestamp).collect::<Vec<_>>();
        let price_cols = data.iter().map(|x| x.price).collect::<Vec<_>>();
        let vol_cols = data.iter().map(|x| x.volume).collect::<Vec<_>>();
        let from_cols = data
            .iter()
            .map(|x| ByteArray::from(x.from.as_str()))
            .collect::<Vec<_>>();

        let message_type = "
  message schema {
    REQUIRED INT64 timestamp;
    REQUIRED DOUBLE price;
    REQUIRED DOUBLE volume;
    REQUIRED BINARY msg (UTF8);
  }
";
        let schema = Arc::new(parse_message_type(message_type).unwrap());
        let props = Arc::new(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .build(),
        );
        let file = fs::File::create(&path).unwrap();
        let mut writer = SerializedFileWriter::new(file, schema, props).unwrap();

        let mut row_group_writer = writer.next_row_group().unwrap();
        let mut count = 0;
        while let Some(mut col_writer) = row_group_writer.next_column().unwrap() {
            count += 1;
            match col_writer {
                ColumnWriter::Int64ColumnWriter(ref mut w) => {
                    w.write_batch(&ts_cols, None, None).unwrap();
                }
                ColumnWriter::DoubleColumnWriter(ref mut w) => {
                    if count == 2 {
                        w.write_batch(&price_cols, None, None).unwrap();
                    } else {
                        w.write_batch(&vol_cols, None, None).unwrap();
                    }
                }
                ColumnWriter::ByteArrayColumnWriter(ref mut w) => {
                    w.write_batch(&from_cols, None, None).unwrap();
                }
                _ => unimplemented!(),
            }
            row_group_writer.close_column(col_writer).unwrap();
        }
        writer.close_row_group(row_group_writer).unwrap();
        writer.close().unwrap();
    }
}
