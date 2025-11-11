use clap::CommandFactory;
use pctx::Cli;

fn main() {
    let cli = Cli::command();
    let markdown = clap_markdown::help_markdown_command(&cli);
    println!("{markdown}");
}
