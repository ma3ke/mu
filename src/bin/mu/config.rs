use std::{path::PathBuf, str::FromStr};

use ratatui::style::Color;

#[derive(Debug)]
pub struct Config {
    pub colors: Colors,
    pub show_room: bool,
    pub data_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colors: Default::default(),
            show_room: Default::default(),
            data_path: PathBuf::from("/martini/sshuser/mu/mu.dat"),
        }
    }
}

#[derive(Debug)]
pub struct Colors {
    pub divider: Color,
    // Header.
    pub user: Color,
    pub hostname: Color,
    pub os: Color,
    pub clock: Color,
    pub gauge: Color,
    // Table.
    pub hotness_gradient: Box<[Color]>,
    pub student: Color,
    pub visitor: Color,
    pub reservation: Color,
    pub owner: Color,
    pub room: Color,
    pub cores_active: Color,
    pub cores_divider: Color,
    pub cores_total: Color,
    pub cores_bg: Color,
    pub active_user: Color,
    pub active_task: Color,
    pub active_cores: Color,
    // Gutter.
    pub stats: Color,
    pub notes: Color,
}

impl Colors {
    pub fn pick_gradient_color(&self, load: f64) -> Color {
        let gradient = &self.hotness_gradient;
        let n_colors = gradient.len();
        assert!(n_colors > 0, "at least one gradient color must be defined");
        let idx = ((load * n_colors.saturating_sub(1) as f64) as usize).clamp(0, n_colors - 1);
        gradient[idx]
    }
}

impl Default for Colors {
    fn default() -> Self {
        let hotness_gradient = [
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
        ]
        .into();

        Self {
            divider: Color::Gray,
            user: Color::White,
            hostname: Color::Gray,
            os: Color::DarkGray,
            clock: Color::DarkGray,
            gauge: Color::Red,
            hotness_gradient,
            student: Color::LightCyan,
            visitor: Color::LightMagenta,
            reservation: Color::Gray,
            owner: Color::White,
            room: Color::DarkGray,
            cores_active: Color::from_str("#eeeeee").unwrap(),
            cores_divider: Color::from_str("#aaaaaa").unwrap(),
            cores_total: Color::from_str("#cccccc").unwrap(),
            cores_bg: Color::from_str("#999999").unwrap(),
            active_user: Color::Gray,
            active_task: Color::Gray,
            active_cores: Color::Gray,
            stats: Color::Yellow,
            notes: Color::from_str("#70abaf").unwrap(),
        }
    }
}

mod parse {
    use std::{io::Read, path::Path};

    use anyhow::{Context, Result, bail};

    use crate::config::{Color, Colors, Config};

    impl Config {
        /// Opens, reads, and parses a `.ini` file describing the machines configuration.
        ///
        /// Machines are grouped by their rooms, specified by headers.
        /// Under each header, the machines that belong to that room are listed.
        /// Each machine listing starts with the machine hostname, a colon, a space, and finally the
        /// name or note describing who that machine belongs to.
        pub fn read_from_config(path: impl AsRef<Path>) -> Result<Self> {
            let path = path.as_ref();
            let mut s = String::new();
            std::fs::File::open(path)
                .context(format!("could not open config file at {path:?}"))?
                .read_to_string(&mut s)
                .context(format!("could not read config file at {path:?}"))?;

            let mut config = Config::default();
            let mut lines = s.lines().enumerate().peekable();
            while let Some((ln, line)) = lines.next() {
                let Some(line) = strip_comments(line) else { continue };

                // At this point, any remaining line has no surrounding spaces nor trailing comments.
                if let Some(potential_header) = line.strip_prefix('[')
                    && let Some(header) = potential_header.strip_suffix(']')
                {
                    // A header is surrounded by brackets.
                    let header = header.trim(); // "Tighten up those lines!"
                    match header {
                        "general" => parse_general(&mut lines, &mut config),
                        "colors" => parse_colors(&mut lines, &mut config.colors),
                        unknown => {
                            bail!("encountered an unknown config header on line {ln}: {unknown:?}")
                        }
                    }?
                } else {
                    // Otherwise, we're dealing with an orphan line.
                    bail!("encountered a declaration not under a header at line {ln}: {line:?}")
                }
            }

            Ok(config)
        }
    }

    /// A helper function for formatting parsing errors.
    fn f(ln: usize, value: &str, expected: &str) -> String {
        format!("could not parse {value:?} as {expected} on line {ln}")
    }

    /// A helper function for formatting [`Color`] parsing errors.
    fn c(ln: usize, value: &str) -> String {
        f(ln, value, "color")
    }

    fn parse_general<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'a str)>>,
        config: &mut Config,
    ) -> Result<()> {
        loop {
            // First, we check if we are running into the next header or the end of the file.
            // We leave that to be handled after we return.
            match lines.peek() {
                // Encountered a header. Exiting.
                Some((_ln, line)) if line.trim_start().starts_with('[') => break,
                // We are at the end. Exiting.
                None => break,
                _ => {}
            }

            // Let's take the next line now.
            let (ln, line) = lines.next().unwrap(); // We know it exists.
            let Some(line) = strip_comments(line) else { continue };

            // Now we know that we are dealing with a declaration line.
            let Some((keyword, value)) = line.split_once(char::is_whitespace) else {
                bail!(
                    "expected a declaration of the form 'keyword value' on line {ln}, but found {line:?}"
                );
            };

            match (keyword.trim_end(), value.trim()) {
                ("show_room", value) => {
                    config.show_room = value.parse().context(f(ln, value, "bool"))?
                }
                ("data_path", value) => config.data_path = value.into(),
                (keyword, _) => bail!("unknown keyword {keyword:?} on line {ln}"),
            }
        }

        Ok(())
    }

    fn parse_colors<'a>(
        lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'a str)>>,
        config: &mut Colors,
    ) -> Result<()> {
        loop {
            // First, we check if we are running into the next header or the end of the file.
            // We leave that to be handled after we return.
            match lines.peek() {
                // Encountered a header. Exiting.
                Some((_ln, line)) if line.trim_start().starts_with('[') => break,
                // We are at the end. Exiting.
                None => break,
                _ => {}
            }

            // Let's take the next line now.
            let (ln, line) = lines.next().unwrap(); // We know it exists.
            let Some(line) = strip_comments(line) else { continue };

            // Now we know that we are dealing with a declaration line.
            let Some((keyword, value)) = line.split_once(char::is_whitespace) else {
                bail!(
                    "expected a declaration of the form 'keyword value' on line {ln}, but found {line:?}"
                );
            };

            let a = |ln, value: &str| value.parse::<Color>().context(c(ln, value));
            let value = value.trim();
            let color = a(ln, value);
            match (keyword.trim_end(), color) {
                ("divider", color) => config.divider = color?,
                ("user", color) => config.user = color?,
                ("hostname", color) => config.hostname = color?,
                ("os", color) => config.os = color?,
                ("clock", color) => config.clock = color?,
                ("gauge", color) => config.gauge = color?,
                ("student", color) => config.student = color?,
                ("visitor", color) => config.visitor = color?,
                ("reservation", color) => config.reservation = color?,
                ("owner", color) => config.owner = color?,
                ("room", color) => config.room = color?,
                ("cores_active", color) => config.cores_active = color?,
                ("cores_divider", color) => config.cores_divider = color?,
                ("cores_total", color) => config.cores_total = color?,
                ("cores_bg", color) => config.cores_bg = color?,
                ("active_user", color) => config.active_user = color?,
                ("active_task", color) => config.active_task = color?,
                ("active_cores", color) => config.active_cores = color?,
                ("stats", color) => config.stats = color?,
                ("notes", color) => config.notes = color?,

                // The gradient is a bit tricky.
                ("hotness_gradient", _) => {
                    if value.starts_with('[') {
                        config.hotness_gradient =
                            parse_list(lines).context(f(ln, "[ ... ]", "color list"))?
                    } else {
                        bail!("expected a list starting with '[' at line {ln}, but found {value:?}")
                    }
                }

                // And the catch-all for unknown keywords.
                (keyword, _) => bail!("unknown color keyword {keyword:?} on line {ln}"),
            }
        }

        Ok(())
    }

    fn parse_list<'a>(lines: &mut impl Iterator<Item = (usize, &'a str)>) -> Result<Box<[Color]>> {
        lines
            .take_while(|(_ln, line)| !line.contains(']'))
            .map(|(ln, line)| {
                let value = line.trim();
                value.parse().context(f(ln, value, "color"))
            })
            .collect::<Result<_>>()
    }

    /// Strip any comments.
    ///
    /// Returns [`Some`] line if the line is not empty. If the line is empty,
    /// this function returns [`None`].
    fn strip_comments(line: &str) -> Option<&str> {
        // Strip any comments.
        let line = match line.split_once(';') {
            Some((line, _comment)) => line,
            None => line,
        }
        .trim();
        if line.is_empty() {
            // Skip empty lines and line comments.
            return None;
        }
        Some(line)
    }
}
