use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Buffer, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, LineGauge, Paragraph, Row, Table, Widget, Wrap};
use ratatui::{DefaultTerminal, Frame, symbols};

use crate::config::{Colors, Config};
use crate::view::{ClusterDataView, MachineView};
use mu::model::{ActiveUser, ClusterData, CpuUsage, HostInfo, Memory, Owner};

pub struct App {
    colors: Colors,
    host_info: HostInfo,
    path: PathBuf,
    data: Option<ClusterData>,
    access_logged: bool,
    /// Report if the data was refreshed successfully.
    success: bool,
    show_room: bool,
    #[allow(dead_code)] // TODO
    dirty: bool,
    exit: bool,
}

fn log(host_info: &HostInfo) -> Result<()> {
    const DEFAULT_LOG_PATH: &str = "/martini/sshuser/mu/usage.log";
    let log_path = std::env::var("MU_LOG_PATH").unwrap_or(DEFAULT_LOG_PATH.to_string());
    let mut file = std::fs::File::options().append(true).open(&log_path)?;
    let HostInfo { hostname, user, os, os_version } = host_info;
    let timestamp = chrono::offset::Local::now().to_rfc3339();
    writeln!(file, "{timestamp}\tfrom {user}@{hostname}\t({os} {os_version})")?;
    Ok(())
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let host_info = HostInfo::new()?;
        // Here is something silly: we'll append a line to a log file when mu is used.
        let access_logged = log(&host_info).is_ok();
        Ok(Self {
            colors: config.colors,
            host_info,
            path: config.data_path,
            data: None,
            access_logged,
            success: false,
            show_room: config.show_room,
            dirty: true,
            exit: false,
        })
    }

    pub fn host_info(&self) -> &HostInfo {
        &self.host_info
    }

    /// Before reading, the data must be [refreshed](Self::refresh_data). If this is not the case,
    /// this function may return `None`.
    pub fn data(&self) -> Option<&ClusterData> {
        self.data.as_ref()
    }

    pub fn refresh_data(&mut self) -> Result<&ClusterData> {
        // Reset the success flag.
        self.success = false;
        let data_path = &self.path;
        // TODO: Perhaps we can use a thread_local to re-use the allocation?

        // Read all usage data file contents at once in an attempt to avoid deserializing the file
        // contents while it is being written by `mu-hive`.
        let file = std::fs::read(data_path).context(format!(
            "could not open the path {data_path:?}, try providing a path as an argument"
        ))?;
        let data = serde_json::from_slice(&file)?;
        self.data = Some(data);
        // Report the success.
        self.success = true;
        Ok(self.data().unwrap())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // We load the data a first time return an error if it is not successful.
        self.refresh_data()?;
        while !self.exit {
            // In case subsequent refreshing is not successful, we just wait a bit longer.
            let _ = self.refresh_data();
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
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.exit()
            }
            KeyCode::Char('j') | KeyCode::Down => {}
            KeyCode::Char('k') | KeyCode::Up => {}
            KeyCode::Char('R') => self.show_room = !self.show_room,
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn view(&self) -> ClusterDataView {
        let data = self.data().expect("data must be refreshed before it is read");
        // TODO: This clone could be elided in the future maybe?
        ClusterDataView::new(
            self.host_info.clone(),
            data,
            self.access_logged,
            self.success,
            self.show_room,
        )
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let view = self.view();
        let colors = &self.colors;

        let header_info = {
            let HostInfo { hostname, user, os, os_version } = view.header.hostinfo;
            Line::from(vec![
                Span::from(user).bold().fg(colors.user),
                Span::from(" @ ").fg(colors.divider),
                Span::from(hostname).bold().italic().fg(colors.hostname),
                Span::from(format!(" {os} {os_version}")).fg(colors.os),
            ])
            .left_aligned()
        };
        let time = Span::from(chrono::Local::now().format("%H:%M").to_string())
            .into_centered_line()
            .bold()
            .fg(colors.clock);
        let header_info_width = header_info.width();
        let gauge = LineGauge::default()
            .line_set(symbols::line::THICK)
            .filled_style(Style::new().fg(colors.gauge))
            .unfilled_style(Style::new().dim())
            .ratio(view.header.total_usage.into())
            .block(Block::new());

        let info = Paragraph::new(header_info).wrap(Wrap { trim: true });
        let legend_row = Row::new(["", "", "Room", "CPU", "Mem", "Active process"].map(Cell::from))
            .fg(colors.legend);
        let machines_rows: Vec<Row> = std::iter::once(legend_row)
            .chain(view.machines.into_iter().map(|machine| IntoRow::into_row(machine, colors)))
            .collect();

        let machines = Table::new(
            machines_rows,
            [
                Constraint::Max(6),  // Hostname.
                Constraint::Max(23), // Note (owner).
                if self.show_room { Constraint::Max(9) } else { Constraint::Length(0) }, // Room.
                Constraint::Length(7), // Cores.
                Constraint::Length(10), // Memory.
                Constraint::Max(30), // Active user.
            ],
        )
        .block(Block::new());

        let stats_rows = view
            .stats
            .iter()
            .map(|(user, usage_percent)| {
                Row::new(vec![
                    Cell::from(Span::from(format!("{usage_percent:>3.0}%")))
                        .italic()
                        .fg(colors.stats)
                        .dim(),
                    Cell::from(Span::from(user)).bold(),
                ])
            })
            .collect::<Vec<_>>();
        let stats_block = Block::bordered().title("User ranking").fg(colors.stats);
        let stats_height = stats_rows.len() as u16 + 2;
        let stats =
            Table::new(stats_rows, [Constraint::Length(4), Constraint::Min(8)]).block(stats_block);

        // TODO: Better minute/seconds reporting?
        let age = match view.notes.last_update.elapsed() {
            Ok(age) => format!("{:.0} s ago", age.as_secs_f32()),
            // This would be very weird but it's cool to handle it in a funny way :)
            Err(error) => format!("{:.3} s in the future", error.duration().as_secs_f32()),
        };
        let notes_block = Block::bordered().title("Notes").fg(colors.notes);
        let notes = Paragraph::new(vec![
            Line::from("Last update:"),
            Line::from(format!("  {age}.")),
            Line::from(if view.notes.success { ":)" } else { ":(" }),
            Line::from(if view.notes.logged { "Logged." } else { "Not logged." }),
        ])
        .wrap(Wrap { trim: false })
        .block(notes_block);

        let vertical_layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let header_layout = Layout::horizontal([
            Constraint::Min(header_info_width as u16 + 1), // info
            Constraint::Min(5),                            // time
            Constraint::Max(40),                           // gauge
        ]);
        let main_layout = Layout::horizontal([Constraint::Fill(1), Constraint::Length(18)]);
        let gutter_layout = Layout::vertical([
            Constraint::Max(stats_height),
            Constraint::Max(6),
            Constraint::Fill(1),
        ]);
        let [header_area, main_area] = vertical_layout.areas(area);
        let [info_area, time_area, gauge_area] = header_layout.areas(header_area);
        let [table_area, gutter_area] = main_layout.areas(main_area);
        let [stats_area, notes_area, _rest_area] = gutter_layout.areas(gutter_area);

        info.render(info_area, buf);
        time.render(time_area, buf);
        gauge.render(gauge_area, buf);
        machines.render(table_area, buf);
        stats.render(stats_area, buf);
        notes.render(notes_area, buf);
    }
}

trait IntoRow<'a> {
    fn into_row(self, colors: &Colors) -> Row<'a>;
}

impl<'a> IntoRow<'a> for MachineView {
    fn into_row(self, colors: &Colors) -> Row<'a> {
        let CpuUsage { used, total } = self.cpu_usage;

        let hostname = {
            let text = Span::from(self.hostname);
            let modifier =
                if used == total { Modifier::BOLD | Modifier::ITALIC } else { Modifier::empty() };
            let load = self.load_avg.five / total as f64;
            let color = colors.pick_gradient_color(load);
            Cell::from(text.fg(color).add_modifier(modifier))
        };
        // TODO: Add an owner.name() -> Option<String> thing.
        // We want to know whether the main active user of a machine is also its owner.
        let uses_own = match (&self.owner, &self.active_user) {
            (Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name), Some(au))
                if *name == au.user =>
            {
                Modifier::UNDERLINED
            }
            _ => Modifier::empty(),
        };
        // We also want to know whether a student or visitor's machine is most actively used by
        // somebody else.
        let other_user = match (&self.owner, &self.active_user) {
            (Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name), Some(au))
                if *name != au.user =>
            {
                Modifier::UNDERLINED
            }
            _ => Modifier::empty(),
        };
        let owner_name_style = Style::new().bold().add_modifier(uses_own);
        let owner = match self.owner {
            Owner::Member(name) => Cell::from(Line::from(vec![
                Span::raw("  "),
                Span::raw(name).style(owner_name_style).fg(colors.owner),
            ])),
            Owner::Visitor(name) => Cell::from(Line::from(vec![
                Span::raw("v").italic().fg(colors.visitor).add_modifier(other_user),
                Span::raw(" "),
                Span::raw(name).style(owner_name_style).fg(colors.owner),
            ])),
            Owner::Student(name) => Cell::from(Line::from(vec![
                Span::raw("s").italic().fg(colors.student).add_modifier(other_user),
                Span::raw(" "),
                Span::raw(name).style(owner_name_style).fg(colors.owner),
            ])),
            Owner::Reserve => {
                Cell::from(Span::raw("Reservation required").italic().fg(colors.reservation))
            }
            Owner::None => Cell::default(),
        };
        let cpu = {
            let u = self.load_avg.one.round() as u32;
            Cell::from(Line::from(vec![
                Span::raw(format!("{u:>3}")).fg(colors.cores_active).bold(),
                Span::raw("/").fg(colors.cores_divider),
                Span::raw(format!("{total:<3}")).fg(colors.cores_total).bold(),
            ]))
            .bg(colors.cores_bg)
        };
        let mem = {
            let Memory { used, total } = self.mem_usage;
            let length = 5;
            let nfilled = ((used * length) / total) as usize;
            let filled = symbols::line::THICK_HORIZONTAL.repeat(nfilled);
            let empty = symbols::line::HORIZONTAL.repeat(length as usize - nfilled);
            Cell::from(Line::from(vec![
                Span::raw(filled).fg(colors.divider),
                Span::raw(empty).fg(colors.divider).dim(),
            ]))
        };
        let active_user = if let Some(ActiveUser { user, cores, task }) = self.active_user {
            let mut line = Line::from(vec![
                Span::raw(format!("{user:>8}")).bold().fg(colors.active_user),
                Span::raw(":").fg(colors.divider).dim(),
                Span::raw(task).italic().fg(colors.active_task),
            ]);
            if cores > 1 {
                line.extend([
                    Span::raw("@").fg(colors.divider).dim(),
                    Span::raw(cores.to_string()).bold().fg(colors.active_cores),
                ]);
            }
            Cell::from(line)
        } else {
            Cell::default() // If there is no active user process we leave the cell empty.
        };

        Row::new(vec![
            hostname,
            owner,
            if self.show_room {
                Cell::from(Text::from(self.room).right_aligned()).fg(colors.room)
            } else {
                Cell::default() // Empty.
            },
            cpu,
            mem,
            active_user,
        ])
    }
}
