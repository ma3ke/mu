use std::collections::HashMap;
use std::io::Read;
use std::str::FromStr;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Buffer, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, LineGauge, Paragraph, Row, Table, Widget, Wrap};
use ratatui::{DefaultTerminal, Frame};
use sysinfo::System;

use crate::data::Data;

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
            Cell::from(Line::from(vec![
                Span::raw(format!("{user:>8}")).bold().gray(),
                Span::raw(":").dim(),
                Span::raw(task).italic(),
                Span::raw("@").dim(),
                Span::raw(cores.to_string()).bold().gray(),
            ]))
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
    hostname: String,
    user: String,
    os: String,
    os_version: String,
    data: Data,
    exit: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
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
        let App {
            hostname,
            user,
            os,
            os_version,
            data,
            exit: _,
        } = self;

        let machines = {
            let mut ms = data.machines();
            ms.sort_by_cached_key(|m| m.hostname.clone());
            ms
        };

        // TODO: Move to a method on Data.
        // TODO: Also rewrite this this sucks.
        let mut tpu = HashMap::<_, usize>::new();
        let mut cpu_count = 0;
        for entry in &data.info.0 {
            cpu_count += entry.cpu_usage.total;
            for (user, cu) in &entry.usage {
                // TODO: I think this is a cursed way of counting total usage.
                *tpu.entry(user).or_default() += cu.len();
            }
        }
        // TODO: This should happen at the App update level, not during rendering.
        let tpu = {
            let mut tpu: Vec<(&String, usize)> = tpu.into_iter().collect();
            tpu.sort_by_key(|(_, tasks_sum)| *tasks_sum);
            tpu
        };

        let header_info = Line::from(vec![
            Span::from(user).bold(),
            Span::from(" @ ").dim(),
            Span::from(hostname).bold().italic().gray(),
            Span::from(format!(" {os} {os_version}")).dim(),
        ])
        .left_aligned();
        let header_info_width = header_info.width();
        let total_usage = {
            let total_cores_used: u32 = data.info.0.iter().map(|entry| entry.cpu_usage.used).sum();
            let total_cores: u32 = data.info.0.iter().map(|entry| entry.cpu_usage.total).sum();
            total_cores_used as f64 / total_cores as f64
        };
        let gauge = LineGauge::default()
            .filled_style(Style::new().light_red())
            .unfilled_style(Style::new().dark_gray().dim())
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
                Constraint::Max(22),
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
        .unwrap_or("/martini/sshuser/machine_usage/machine_usage.dat".to_string());

    let mut s = String::new();
    std::fs::File::open(&data_path)
        .context(format!(
            "could not open the path {data_path:?}, try providing a path as an argument"
        ))?
        .read_to_string(&mut s)?;
    let data = Data::parse(&s)?;

    let mut app = App {
        hostname: hostname::get()?.to_str().unwrap_or("?").to_string(),
        user: users::get_current_username()
            .map(|u| u.to_string_lossy().to_string())
            .unwrap_or("?".to_string()),
        os: System::name().unwrap_or("?".to_string()),
        os_version: System::os_version().unwrap_or("?".to_string()),
        data,
        exit: false,
    };
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
