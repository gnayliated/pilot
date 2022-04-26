use chrono::Duration;
use chrono::RoundingError::DurationExceedsLimit;
use octorust::auth::Credentials;
use octorust::gists::Gists;
use octorust::types::{GistsCreateRequest, PublicOneOf};
use octorust::{gists, Client};
use parquet::column::writer::ColumnWriter;
use parquet::{
    file::{
        properties::WriterProperties,
        writer::{FileWriter, SerializedFileWriter},
    },
    schema::parser::parse_message_type,
};
use pilot_pipeline::orderbook::{OrderbookConfig, PilotOrderBook};
use std::str::FromStr;
use std::{fs, path::Path, sync::Arc};

#[derive(clap::Parser, Debug)]
pub struct Command {
    #[clap(long, default_value = "")]
    pub gist_token: String,

    #[clap(long)]
    pub orderbook_path: Option<String>,
}

pub struct Uploader {
    cmd: Command,
}

impl Uploader {
    pub fn new(cmd: Command) -> Self {
        Self { cmd }
    }

    pub fn run(&self) {
        if !self.cmd.orderbook_path.is_none() {
            self.load_gist_orderbook();
        }
    }

    pub fn load_gist_orderbook(&self) {
        let gist_token = self.cmd.gist_token.clone();
        let path = self.cmd.orderbook_path.as_ref().unwrap();
        let cfg = OrderbookConfig::from_str(path).unwrap();

        // TODO: use github repo as store

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let github =
                    Client::new(String::from("pilot-upload"), Credentials::Token(gist_token))
                        .unwrap();
                let g = gists::Gists::new(github);

                let now = chrono::Utc::now() - Duration::hours(1);
                let yesterday = now - Duration::days(1);

                for i in cfg.symbols {
                    let description = format!("{}-{}", i.symbol, now);

                    let values = g.list_all(Some(yesterday)).await.unwrap();
                    for i in values.iter().filter(|g| g.created_at.unwrap() < now) {
                        Self::gist_comments_to_parquet(&g, &i.id).await;
                    }
                }
            })
    }

    async fn gist_comments_to_parquet(g: &Gists, id: &str) {
        let comments = g.list_comments(id, 100, 0).await.unwrap();

        let mut created_vec = vec![];
        let mut price_vec = vec![];
        let mut volume_vec = vec![];
        comments.iter().for_each(|c| {
            let ob: PilotOrderBook = serde_json::from_str(&c.body).unwrap();

            log::debug!(
                "orderbook:{} bids={} asks{} created={}",
                ob.symbol,
                ob.bids.len(),
                ob.asks.len(),
                ob.created
            );

            for i in ob.asks {
                created_vec.push(ob.created);
                price_vec.push(i.price);
                volume_vec.push(i.volume);
            }

            for i in ob.bids {
                created_vec.push(ob.created);
                price_vec.push(i.price);
                volume_vec.push(i.volume);
            }
        });

        log::info!(
            "Loading comments id={} comments={} created={} price={} volume={}",
            id,
            comments.len(),
            created_vec.len(),
            price_vec.len(),
            volume_vec.len(),
        );

        let name = format!("./{}.parquet", id);
        let path = Path::new(&name);

        let message_type = "
  message schema {
    REQUIRED INT64 timestamp;
    REQUIRED DOUBLE price;
    REQUIRED DOUBLE volume;
  }
";
        let schema = Arc::new(parse_message_type(message_type).unwrap());
        let props = Arc::new(WriterProperties::builder().build());
        let file = fs::File::create(&path).unwrap();
        let mut writer = SerializedFileWriter::new(file, schema, props).unwrap();

        let mut row_group_writer = writer.next_row_group().unwrap();
        let mut count = 0;
        while let Some(mut col_writer) = row_group_writer.next_column().unwrap() {
            count += 1;
            match col_writer {
                ColumnWriter::Int64ColumnWriter(ref mut w) => {
                    w.write_batch(&created_vec, None, None).unwrap();
                }
                ColumnWriter::DoubleColumnWriter(ref mut w) => {
                    if count == 2 {
                        w.write_batch(&price_vec, None, None).unwrap();
                    } else {
                        w.write_batch(&volume_vec, None, None).unwrap();
                    }
                }
                _ => unimplemented!(),
            }
            row_group_writer.close_column(col_writer).unwrap();
        }
        writer.close_row_group(row_group_writer).unwrap();
        writer.close().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_parquet() {
        let path = Path::new("./sample.parquet");

        let message_type = "
  message schema {
    REQUIRED INT64 timestamp;
  }
";
        let schema = Arc::new(parse_message_type(message_type).unwrap());
        let props = Arc::new(WriterProperties::builder().build());
        let file = fs::File::create(&path).unwrap();
        let mut writer = SerializedFileWriter::new(file, schema, props).unwrap();

        let mut row_group_writer = writer.next_row_group().unwrap();
        while let Some(mut col_writer) = row_group_writer.next_column().unwrap() {
            match col_writer {
                ColumnWriter::Int64ColumnWriter(ref mut w) => {
                    w.write_batch(&vec![1, 2, 3, 4, 5, 6, 7], None, None)
                        .unwrap();
                }
                _ => unimplemented!(),
            }
            row_group_writer.close_column(col_writer).unwrap();
        }
        writer.close_row_group(row_group_writer).unwrap();
        writer.close().unwrap();
    }
}
