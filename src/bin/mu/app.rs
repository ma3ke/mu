use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Buffer, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, LineGauge, Paragraph, Row, Table, Widget, Wrap};
use ratatui::{DefaultTerminal, Frame, symbols};

use mu::info::{Data, HostInfo};

use crate::data::DataView;

pub struct App {
    host_info: HostInfo,
    path: PathBuf,
    data: Option<Data>,
    #[allow(dead_code)] // TODO
    dirty: bool,
    exit: bool,
}

impl App {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        Ok(Self {
            host_info: HostInfo::new()?,
            path: path.as_ref().to_path_buf(),
            data: None,
            dirty: true,
            exit: false,
        })
    }

    pub fn host_info(&self) -> &HostInfo {
        &self.host_info
    }

    /// Before reading, the data must be [refreshed](Self::refresh_data). If this is not the case,
    /// this function may return `None`.
    pub fn data(&self) -> Option<&Data> {
        self.data.as_ref()
    }

    pub fn refresh_data(&mut self) -> Result<&Data> {
        let data_path = &self.path;
        // TODO: Perhaps we can use a thread_local to re-use the allocation?

        // Read all usage data file contents at once in an attempt to avoid deserializing the file
        // contents while it is being written by `mu-hive`.
        let file = std::fs::read(data_path).context(format!(
            "could not open the path {data_path:?}, try providing a path as an argument"
        ))?;
        let data = serde_json::from_slice(&file)?;
        self.data = Some(data);
        Ok(self.data().unwrap())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            self.refresh_data()?; // TODO: Cursed because we shouldn't update every frame.
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(1000))? {
            match event::read()? {
                // it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            };
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('j') | KeyCode::Down => {}
            KeyCode::Char('k') | KeyCode::Up => {}
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let HostInfo {
            hostname,
            user,
            os,
            os_version,
        } = self.host_info();

        let data = self
            .data()
            .expect("data must be refreshed before it is read");
        let total_usage = data.total_usage();
        let machines = data.machines();
        let tpu = data.tpu();
        let cpu_count = data.cpu_count();

        let header_info = Line::from(vec![
            Span::from(user).bold(),
            Span::from(" @ ").dim(),
            Span::from(hostname).bold().italic().gray(),
            Span::from(format!(" {os} {os_version}")).dim(),
        ])
        .left_aligned();
        let header_info_width = header_info.width();
        let gauge = LineGauge::default()
            .line_set(symbols::line::THICK)
            .filled_style(Style::new().red())
            .unfilled_style(Style::new().dim())
            .ratio(total_usage)
            .block(Block::new());

        let header = Paragraph::new(header_info).wrap(Wrap { trim: true });
        let machines_rows: Vec<Row> = machines
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let r: Row = m.into();
                // TODO: Pick something here.
                if i % 2 == 0 { r } else { r }
            })
            .collect();
        let machines = Table::new(
            machines_rows,
            [
                Constraint::Max(6),
                Constraint::Max(23),
                Constraint::Max(9),
                Constraint::Length(7),
                Constraint::Max(30),
            ],
        )
        .block(Block::new());

        let stats_rows: Vec<_> = tpu
            .iter()
            .rev()
            .flat_map(|&(user, tasks_sum)| {
                let usage_percent = 100.0 * tasks_sum as f64 / cpu_count as f64;
                if usage_percent < 1.0 {
                    None
                } else {
                    let usage_percent = match cpu_count {
                        0 => " ?? %".to_string(),
                        _ => format!("{usage_percent:>3.0}%"),
                    };
                    Some(Row::new(vec![
                        Cell::from(Span::from(usage_percent)).italic().dim(),
                        Cell::from(Span::from(user)).bold(),
                    ]))
                }
            })
            .collect();
        let stats_block = Block::bordered().title("User ranking").yellow();
        let stats_height = stats_rows.len() as u16 + 2;
        let stats =
            Table::new(stats_rows, [Constraint::Length(4), Constraint::Min(8)]).block(stats_block);

        // TODO: Better minute/seconds reporting?
        let age = match data.time().elapsed() {
            Ok(age) => format!("{:.0} s ago", age.as_secs_f32()),
            // This would be very weird but it's cool to handle it in a funny way :)
            Err(error) => format!("{:.3} s in the future", error.duration().as_secs_f32()),
        };
        let notes_block = Block::bordered()
            .title("Notes")
            .fg(Color::from_str("#70abaf").unwrap());
        let notes = Paragraph::new(format!("Last update:\n  {age}."))
            .wrap(Wrap { trim: false })
            .block(notes_block);

        let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let header_layout = Layout::horizontal([
            Constraint::Min(header_info_width as u16 + 1),
            Constraint::Max(50),
        ]);
        let main_layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(18)]);
        let gutter_layout = Layout::vertical([
            Constraint::Max(stats_height),
            Constraint::Max(4),
            Constraint::Fill(1),
        ]);
        let [header_area, main_area] = vertical_layout.areas(area);
        let [title_area, gauge_area] = header_layout.areas(header_area);
        let [table_area, gutter_area] = main_layout.areas(main_area);
        let [stats_area, notes_area, _rest_area] = gutter_layout.areas(gutter_area);

        header.render(title_area, buf);
        gauge.render(gauge_area, buf);
        machines.render(table_area, buf);
        stats.render(stats_area, buf);
        notes.render(notes_area, buf);
    }
}
