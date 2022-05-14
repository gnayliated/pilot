use clap::Parser;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Arbitrage(pilot_arbitrage::arbitrage::ArbitrageCommand),
    FetchPrice(pilot_pipeline::fetch_price::PriceCommand),
    FetchOrderbook(pilot_pipeline::fetch_orderbook::OrderbookCommand),
    LoadOrderbook(pilot_pipeline::load_orderbook::OrderbookCommand),
    RemoveOrderbook(pilot_pipeline::remove_orderbook::OrderbookCommand),
    Upload(pilot_upload::Command),
}

fn main() {
    env_logger::init();

    let args = Args::parse();

    println!("{:?}", args);

    match args.action {
        Action::FetchPrice(pc) => {
            let pf = pilot_pipeline::fetch_price::PriceFetcher::new(pc);
            pf.run();
        }
        Action::FetchOrderbook(oc) => {
            let pf = pilot_pipeline::fetch_orderbook::OrderbookFetcher::new(oc);
            pf.run();
        }
        Action::RemoveOrderbook(oc) => {
            let pf = pilot_pipeline::remove_orderbook::OrderbookRemover::new(oc);
            pf.run();
        }
        Action::LoadOrderbook(oc) => {
            let pf = pilot_pipeline::load_orderbook::OrderbookLoader::new(oc);
            pf.run();
        }
        Action::Upload(cmd) => {
            let u = pilot_upload::Uploader::new(cmd);
            u.run();
        }
        Action::Arbitrage(cmd) => {
            let a = pilot_arbitrage::arbitrage::Arbitrage::new(cmd);
            a.run();
        }
    }
}
