use std::str::FromStr;

use anyhow::Result;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Cell, Row};

use mu::info::LoadAvg;

use app::App;

mod app;
mod data; // TODO: Rename?

struct Machine {
    hostname: String,
    owner: Owner,
    room: String,
    cpu_usage: CpuUsage,
    load_avg: LoadAvg,
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
            load_avg,
            active_user,
        } = self;

        let hostname = {
            let text = Span::from(hostname);
            let t = load_avg.five / total as f64;
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
            let idx = ((t * gradient.len().saturating_sub(1) as f64) as usize)
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
        // We want to know whether the main active user of a machine is also its owner.
        let uses_own = match (&owner, &active_user) {
            (Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name), Some(au))
                if *name == au.user =>
            {
                Modifier::UNDERLINED
            }
            _ => Modifier::empty(),
        };
        // We also want to know whether a student or visitor's machine is most actively used by
        // somebody else.
        let other_user = match (&owner, &active_user) {
            (Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name), Some(au))
                if *name != au.user =>
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
                Span::raw("v")
                    .italic()
                    .light_cyan()
                    .dim()
                    .add_modifier(other_user),
                Span::raw(" "),
                Span::raw(name).style(owner_name_style),
            ])),
            Owner::Student(name) => Cell::from(Line::from(vec![
                Span::raw("s")
                    .italic()
                    .light_magenta()
                    .dim()
                    .add_modifier(other_user),
                Span::raw(" "),
                Span::raw(name).style(owner_name_style),
            ])),
            Owner::Reserve => Cell::from(Span::raw("Reservation required").italic().gray()),
            Owner::None => Cell::default(),
        };
        let cpu = {
            let bg = Color::from_str("#999999").unwrap();
            let bright = Color::from_str("#eeeeee").unwrap();
            let dim = Color::from_str("#cccccc").unwrap();
            let u = load_avg.one.round() as u32;
            Cell::from(Line::from(vec![
                // Span::raw(format!("{used:>3}")).fg(bright).bold(),
                Span::raw(format!("{u:>3}")).fg(bright).bold(),
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
            Cell::from(Text::from(room).right_aligned()).dim(),
            cpu,
            active_user,
        ])
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
