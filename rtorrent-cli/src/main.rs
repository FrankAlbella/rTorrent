use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "snake_case")]
enum Command {
    Add { value: String },
    Remove { value: String },
    Info { value: String },
    List { value: String },
}

fn main() {
    let args = Args::parse();

    match args.command {
        x => todo!("{x:#?}"),
    }
}
