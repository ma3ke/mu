use std::{io::Read, path::Path, str::FromStr};

use anyhow::{Context, Result};
use mu::model::Owner;

#[derive(Debug, Clone)]
pub struct MachineDefinitions(Box<[MachineDefinition]>);

#[derive(Debug, Clone)]
pub struct MachineDefinition {
    pub room: String,
    pub hostname: String,
    /// Note or name of owner.
    ///
    /// Not all machines have such information associated with them.
    pub note: Option<String>,
}

impl From<MachineDefinition> for mu::model::MachineDefinition {
    fn from(definition: MachineDefinition) -> Self {
        let MachineDefinition { room, hostname, note } = definition;
        let owner = note.map(|note| Owner::from_str(&note).unwrap()).unwrap_or_default();
        Self { hostname, owner, room }
    }
}

impl std::ops::Deref for MachineDefinitions {
    type Target = [MachineDefinition];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MachineDefinitions {
    /// Opens, reads, and parses a `.ini` file describing the machines configuration.
    ///
    /// Machines are grouped by their rooms, specified by headers.
    /// Under each header, the machines that belong to that room are listed.
    /// Each machine listing starts with the machine hostname, a colon, a space, and finally the
    /// name or note describing who that machine belongs to.
    pub fn read_from_config(path: impl AsRef<Path>) -> Result<MachineDefinitions> {
        let path = path.as_ref();
        let mut s = String::new();
        std::fs::File::open(path)
            .context(format!("could not open machines file"))?
            .read_to_string(&mut s)
            .context(format!("could not read machines file"))?;

        let mut machines_config = Vec::new();
        let mut room = None;
        let mut lines = s.lines();
        while let Some(line) = lines.next() {
            // Strip any comments.
            let line = match line.split_once('#') {
                Some((line, _comment)) => line,
                None => line,
            }
            .trim();
            if line.is_empty() {
                // Skip empty lines and line comments.
                continue;
            }

            // At this point, any remaining line has no surrounding spaces nor trailing comments.

            if let Some(potential_header) = line.strip_prefix('[')
                && let Some(header) = potential_header.strip_suffix(']')
            {
                // A room header is surrounded by brackets.
                let header = header.trim(); // "Tighten up those lines!"
                room = Some(header);
            } else {
                // Otherwise, we're dealing with a machine line.
                // TODO: Consider whether we want to error out on that or just provide the 'orphan' placeholder.
                let room = room.unwrap_or("orphan").to_string();
                // Parse out the hostname and note/owner information. If the line does not have the
                // expected format, give a warning and skip the line.
                let Some((hostname, note)) = line.split_once(':') else {
                    continue;
                };
                let hostname = hostname.trim().to_string();
                // TODO: Is there a more elegant way to do this? Like an inverse Default::default()?
                let note = match note.trim() {
                    "" => None,
                    note => Some(note.to_string()),
                };
                let machine = MachineDefinition { room, hostname, note };
                machines_config.push(machine);
            }
        }

        Ok(Self(machines_config.into_boxed_slice()))
    }
}
