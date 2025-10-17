use std::path::PathBuf;

use anyhow::Result;

use app::App;
use clap::Parser;

use crate::config::Config;

mod app;
mod config;
mod view;

#[derive(Debug, clap::Parser)]
struct Options {
    /// Path to the configuration file.
    #[clap(long = "config", default_value = "~/.config/mu/mu.conf")]
    config_path: PathBuf,
    /// Path to the `mu.dat` file.
    ///
    /// This will overwrite the default data path or the data path set in the configuration file.
    #[clap(long = "data")]
    data_path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let options = Options::parse();
    let mut config = if options.config_path.exists() {
        Config::read_from_config(options.config_path)?
    } else {
        Config::default()
    };

    // If provided as an argument, overwrite the `data_path`.
    if let Some(data_path) = options.data_path {
        config.data_path = data_path;
    }

    let mut app = App::new(config)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
