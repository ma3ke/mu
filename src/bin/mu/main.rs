use anyhow::Result;

use app::App;

mod app;
mod view;

fn main() -> Result<()> {
    let data_path =
        std::env::args().skip(1).next().unwrap_or("/martini/sshuser/mu/mu.dat".to_string());

    let mut app = App::new(data_path)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
