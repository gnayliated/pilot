use clap::Parser;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Pipeline(pilot_pipeline::Command),
    Upload(pilot_upload::Command),
}

fn main() {
    env_logger::init();

    let args = Args::parse();

    println!("{:?}", args);

    match args.action {
        Action::Pipeline(cmd) => {
            let pipeline = pilot_pipeline::Pipeline::new(cmd);
            pipeline.run();
        }
        Action::Upload(cmd) => {
            let u = pilot_upload::Uploader::new(cmd);
            u.run();
        }
    }
}
