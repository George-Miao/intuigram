//! Intuigram executable entrypoint.

mod cli;

#[compio::main]
async fn main() {
    intuigram_app::main(cli::parse()).await;
}
