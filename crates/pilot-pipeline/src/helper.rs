use crate::fetch_price::PriceCommand;
use crate::orderbook::PilotOrderBook;
use binance::model::PriceStats;
use hyper::{Body, Client};
use pilot_proto::proto::metric_metadata;
use pilot_proto::proto::Label;
use pilot_proto::proto::MetricMetadata;
use pilot_proto::proto::Sample;
use pilot_proto::proto::TimeSeries;
use pilot_proto::proto::WriteRequest;
use serde::Serialize;
use std::fs::File;
use std::io::BufRead;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub struct Symbols(Vec<String>);

impl std::ops::Deref for Symbols {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn parse_lines(s: &str) -> anyhow::Result<Symbols> {
    if s.is_empty() {
        return Ok(Symbols(Vec::new()));
    }

    let file = File::open(s)?;
    let lines = std::io::BufReader::new(file)
        .lines()
        .into_iter()
        .map(|line| line)
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(Symbols(lines))
}

#[derive(Serialize)]
pub struct LCOrderBook {
    requests: Vec<LCOrderBookRequest>,
}

#[derive(Serialize)]
pub struct LCOrderBookRequest {
    method: String,
    path: String,
    body: LCOrderBookRequestBody,
}

#[derive(Serialize)]
pub struct LCOrderBookRequestBody {
    ask: f64,
    bid: f64,
    price: f64,
    created: i64,
    from: String,
}

#[derive(Clone, Debug)]
pub struct LCOption {
    pub class_uri: String,
    pub uri: String,
    pub id: String,
    pub key: String,
    pub from: String,
}

impl From<(LCOption, PilotOrderBook)> for LCOrderBook {
    fn from(p: (LCOption, PilotOrderBook)) -> Self {
        let opt = p.0;
        let path = &opt.class_uri;
        let created = p.1.created;
        let mut body =
            p.1.asks
                .iter()
                .map(|x| LCOrderBookRequestBody {
                    ask: x.volume,
                    bid: 0.0,
                    price: x.price,
                    created: created,
                    from: opt.from.clone(),
                })
                .map(|lc| LCOrderBookRequest {
                    path: path.clone(),
                    method: "POST".to_owned(),
                    body: lc,
                })
                .collect::<Vec<_>>();
        let bids_body =
            p.1.bids
                .iter()
                .map(|x| LCOrderBookRequestBody {
                    bid: x.volume,
                    ask: 0.0,
                    price: x.price,
                    created: created,
                    from: opt.from.clone(),
                })
                .map(|lc| LCOrderBookRequest {
                    path: path.clone(),
                    method: "POST".to_owned(),
                    body: lc,
                })
                .collect::<Vec<_>>();

        body.extend(bids_body);

        LCOrderBook { requests: body }
    }
}

pub(crate) fn post_orderbook_leancloud(opt: LCOption, ob: PilotOrderBook) -> anyhow::Result<u16> {
    let client = reqwest::blocking::Client::new();
    let body: LCOrderBook = LCOrderBook::from((opt.clone(), ob));
    let body = serde_json::to_vec(&body).map_err(|e| anyhow::anyhow!("{}", e))?;
    let res = client
        .post(&opt.uri)
        .header("User-Agent", "pilot/1.0")
        .header("X-LC-Id", &opt.id)
        .header("X-LC-Key", &opt.key)
        .header("Content-Type", "application/json")
        .body(reqwest::blocking::Body::from(body))
        .send()
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    let status = res.status();
    log::debug!("response={:?}", res);
    let text = res.text();
    log::debug!("response body={:?}", text);

    Ok(status.as_u16())
}

pub(crate) fn write_remote_prom(cmd: &PriceCommand, wq: WriteRequest) -> anyhow::Result<u16> {
    let buf = pilot_proto::serialize_write_request(&wq);
    let mut enc = snap::raw::Encoder::new();
    let mut output = vec![0; snap::raw::max_compress_len(buf.len())];
    let n = enc
        .compress(&buf, &mut output)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    output.drain(n..);

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&cmd.endpoint_uri)
        .header("User-Agent", "pilot/1.0")
        .header("Content-Encoding", "snappy")
        .header("Content-Type", "application/x-protobuf")
        .header("X-Prometheus-Remote-Write-Version", "0.1.0")
        .basic_auth(&cmd.endpoint_auth_user, Some(&cmd.endpoint_auth_pass))
        .body(reqwest::blocking::Body::from(output))
        .send()
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    let status = res.status();
    log::debug!("response={:?}", res);
    let text = res.text();
    log::debug!("response body={:?}", text);

    Ok(status.as_u16())
}

pub(crate) fn build_remote_from_stats(stats: &Vec<PriceStats>) -> WriteRequest {
    let now = chrono::offset::Local::now();

    let mut btc_price_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("BTC"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "btc_price".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.last_price,
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let usdt_price_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("USDT"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "usdt_price".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.last_price,
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let btc_trades_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("BTC"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "btc_trades".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.count as f64,
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let usdt_trades_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("USDT"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "usdt_trades".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.count as f64,
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let btc_changes_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("BTC"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "btc_change_percent".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.price_change_percent.parse::<f64>().unwrap_or_default(),
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let usdt_changes_timeseries = stats
        .iter()
        .filter(|s| s.symbol.ends_with("USDT"))
        .map(|stat| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: stat.symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: "usdt_change_percent".to_string(),
                },
            ];

            let sample = Sample {
                value: stat.price_change_percent.parse::<f64>().unwrap_or_default(),
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let metadata = [
        "btc_price",
        "usdt_price",
        "btc_trades",
        "usdt_trades",
        "btc_change_percent",
        "usdt_change_percent",
    ]
    .iter()
    .map(|x| MetricMetadata {
        r#type: metric_metadata::MetricType::Gauge as i32,
        metric_family_name: x.to_string(),
        help: "".to_string(),
        unit: "".to_string(),
    })
    .collect::<Vec<_>>();

    btc_price_timeseries.extend(usdt_price_timeseries);
    btc_price_timeseries.extend(btc_trades_timeseries);
    btc_price_timeseries.extend(usdt_trades_timeseries);
    btc_price_timeseries.extend(btc_changes_timeseries);
    btc_price_timeseries.extend(usdt_changes_timeseries);

    WriteRequest {
        timeseries: btc_price_timeseries,
        metadata,
    }
}

pub(crate) fn build_remote_symbols(name: &str, symbols: Vec<(String, f64)>) -> WriteRequest {
    let now = chrono::offset::Local::now();

    let timeseries = symbols
        .iter()
        .map(|(symbol, value)| {
            let labels = vec![
                Label {
                    name: "symbol".to_string(),
                    value: symbol.to_string(),
                },
                Label {
                    name: "__name__".to_string(),
                    value: name.to_string(),
                },
            ];

            let sample = Sample {
                value: *value,
                timestamp: now.timestamp_millis(),
            };

            let ts = TimeSeries {
                /// For a timeseries to be valid, and for the samples and exemplars
                /// to be ingested by the remote system properly, the labels field is required.
                labels,
                samples: vec![sample],
                exemplars: vec![],
            };
            ts
        })
        .collect::<Vec<_>>();

    let meta = MetricMetadata {
        r#type: metric_metadata::MetricType::Gauge as i32,
        metric_family_name: name.to_string(),
        help: "".to_string(),
        unit: "".to_string(),
    };

    WriteRequest {
        timeseries: timeseries,
        metadata: vec![meta],
    }
}
