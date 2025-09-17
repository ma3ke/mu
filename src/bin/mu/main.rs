use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Buffer, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, LineGauge, Paragraph, Row, Table, Widget, Wrap};
use ratatui::{DefaultTerminal, Frame, symbols};

use mu::HostInfo;
use mu::info::Data;

use data::DataView;

mod data; // TODO: Rename?

struct Machine {
    hostname: String,
    owner: Owner,
    room: String,
    cpu_usage: CpuUsage,
    active_user: Option<ActiveUser>,
}

#[derive(Debug, Clone)]
enum Owner {
    Member(String),
    Visitor(String),
    Student(String),
    Reserve,
    None,
}

impl FromStr for Owner {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(Self::None);
        }
        if s == "Reservation Required" {
            return Ok(Self::Reserve);
        }
        if let Some(name) = s.strip_suffix("(Student)") {
            return Ok(Self::Student(name.trim_end().to_string()));
        }
        if let Some(name) = s.strip_suffix("(Visitor)") {
            return Ok(Self::Visitor(name.trim_end().to_string()));
        }

        Ok(Self::Member(s.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
struct CpuUsage {
    used: u32,
    total: u32,
}

struct ActiveUser {
    user: String,
    cores: u32,
    task: String,
}

impl<'a> Into<Row<'a>> for Machine {
    fn into(self) -> Row<'a> {
        let Self {
            hostname,
            owner,
            room,

            cpu_usage: CpuUsage { used, total },
            active_user,
        } = self;

        let hostname = {
            let text = Span::from(format!("{hostname}"));
            let t = used as f32 / total as f32;
            // TODO: Make a const from this?
            let gradient = [
                Color::from_str("#b0cd75").unwrap(),
                Color::from_str("#c0cc6c").unwrap(),
                Color::from_str("#cfcb63").unwrap(),
                Color::from_str("#d9cf69").unwrap(),
                Color::from_str("#e3d26f").unwrap(),
                Color::from_str("#d7ae67").unwrap(),
                Color::from_str("#ca895f").unwrap(),
                Color::from_str("#c56355").unwrap(),
                Color::from_str("#bf3d4a").unwrap(),
                Color::from_str("#c41829").unwrap(),
            ];
            let idx = ((t * gradient.len().saturating_sub(1) as f32) as usize)
                .clamp(0, gradient.len() - 1);
            let color = gradient[idx];

            let modifier = if used == total {
                Modifier::BOLD | Modifier::ITALIC
            } else {
                Modifier::empty()
            };
            Cell::from(text.fg(color).add_modifier(modifier))
        };
        // TODO: Add an owner.name() -> Option<String> thing.
        let uses_own = match (&owner, &active_user) {
            (Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name), Some(au))
                if *name == au.user =>
            {
                Modifier::UNDERLINED
            }
            _ => Modifier::empty(),
        };
        let owner_name_style = Style::new().bold().add_modifier(uses_own);
        let owner = match owner {
            Owner::Member(name) => Cell::from(Line::from(vec![
                Span::raw("  "),
                Span::raw(name).style(owner_name_style),
            ])),
            Owner::Visitor(name) => Cell::from(Line::from(vec![
                Span::raw("v ").italic().light_cyan().dim(),
                Span::raw(name).style(owner_name_style),
            ])),
            Owner::Student(name) => Cell::from(Line::from(vec![
                Span::raw("s ").italic().light_magenta().dim(),
                Span::raw(name).style(owner_name_style),
            ])),
            Owner::Reserve => Cell::from(Span::raw("Reservation required").italic().gray()),
            Owner::None => Cell::default(),
        };
        let cpu = {
            let bg = Color::from_str("#999999").unwrap();
            let bright = Color::from_str("#eeeeee").unwrap();
            let dim = Color::from_str("#cccccc").unwrap();
            Cell::from(Line::from(vec![
                Span::raw(format!("{used:>3}")).fg(bright).bold(),
                Span::raw("/").dim().fg(dim),
                Span::raw(format!("{total:<3}")).fg(dim).bold(),
            ]))
            .bg(bg)
        };
        let active_user = if let Some(ActiveUser { user, cores, task }) = active_user {
            let mut line = Line::from(vec![
                Span::raw(format!("{user:>8}")).bold().gray(),
                Span::raw(":").dim(),
                Span::raw(task).italic(),
            ]);
            if cores > 1 {
                line.extend([
                    Span::raw("@").dim(),
                    Span::raw(cores.to_string()).bold().gray(),
                ]);
            }
            Cell::from(line)
        } else {
            Cell::default() // If there is no active user process we leave the cell empty.
        };

        Row::new(vec![
            hostname,
            owner,
            Cell::from(Text::from(format!("{room}")).right_aligned()).dim(),
            cpu,
            active_user,
        ])
    }
}

struct App {
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
        let file = std::fs::File::open(data_path).context(format!(
            "could not open the path {data_path:?}, try providing a path as an argument"
        ))?;
        let data = serde_json::from_reader(file)?;
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
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
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
                Constraint::Max(40),
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

        let notes_block = Block::bordered()
            .title("Notes")
            .fg(Color::from_str("#70abaf").unwrap());
        let notes = Paragraph::new("").block(notes_block);

        let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let header_layout = Layout::horizontal([
            Constraint::Min(header_info_width as u16 + 1),
            Constraint::Max(50),
        ]);
        let main_layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(18)]);
        let gutter_layout = Layout::vertical([
            Constraint::Max(stats_height),
            Constraint::Fill(1),
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

fn main() -> Result<()> {
    let data_path = std::env::args()
        .skip(1)
        .next()
        .unwrap_or("/martini/sshuser/mu/mu.dat".to_string());

    let mut app = App::new(data_path)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
