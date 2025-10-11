pub const PROCESS_USAGE_THRESHOLD_PERCENT: f32 = 10.0;

/// Identity of a cluster of machines.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClusterDefinition(Box<[MachineDefinition]>);

/// Identity of a single machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineDefinition {
    pub hostname: String,
    pub owner: Owner,
    pub room: String,
}

/// Usage information for a cluster of machines.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClusterUsage(Box<[MachineUsage]>);

impl ClusterUsage {
    pub fn new(usages: Box<[MachineUsage]>) -> Self {
        Self(usages)
    }

    pub fn cpu_count(&self) -> u32 {
        let mut cpu_count = 0;
        for machine in self.iter() {
            cpu_count += machine.usage.cpus.len();
        }
        cpu_count as u32
    }
}

impl std::ops::Deref for ClusterUsage {
    type Target = [MachineUsage];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Definition and usage information for a single machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineUsage {
    pub definition: MachineDefinition,
    pub usage: Usage,
}

/// Usage information for a single machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    pub global_cpu_usage: f32,
    pub cpus: Box<[f32]>,
    pub load_avg: LoadAvg,
    pub mem: Memory,
    pub processes: Processes,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum Owner {
    Member(String),
    Visitor(String),
    Student(String),
    Reserve,
    #[default]
    None,
}

impl std::str::FromStr for Owner {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub user: String,
    pub os: String,
    pub os_version: String,
}

use std::collections::HashMap;

// TODO: Should this be placed in `mu` because that's the only place where this information is
// actually determined and stored? Right?
use anyhow::Result;
use sysinfo::System;
impl HostInfo {
    /// Create a new [`HostInfo`] describing the current machine.
    pub fn new() -> Result<Self> {
        Ok(Self {
            hostname: hostname::get()?.to_str().unwrap_or("?").to_string(),
            user: users::get_current_username()
                .map(|u| u.to_string_lossy().to_string())
                .unwrap_or("?".to_string()),
            os: System::name().unwrap_or("?".to_string()),
            os_version: System::os_version().unwrap_or("?".to_string()),
        })
    }
}

/// The structure stored in `machine_usage.dat`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClusterData {
    pub timestamp: u64,
    pub usage: ClusterUsage,
}

impl ClusterData {
    /// Creates a new [`ClusterData`].
    ///
    /// The timestamp will be generated from the current time.
    pub fn new(usage: ClusterUsage) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap() // Trust me, we exist after the unix epoch.
            .as_secs();
        Self { timestamp, usage }
    }

    /// Returns the time stored in the timestamp of this [`ClusterData`].
    pub fn time(&self) -> std::time::SystemTime {
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.timestamp)
    }
}

// Direct copy of `sysinfo::LoadAvg` to allow us to easily serialize this information.
#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoadAvg {
    /// Average load within one minute.
    pub one: f64,
    /// Average load within five minutes.
    pub five: f64,
    /// Average load within fifteen minutes.
    pub fifteen: f64,
}

impl From<sysinfo::LoadAvg> for LoadAvg {
    fn from(sysinfo::LoadAvg { one, five, fifteen }: sysinfo::LoadAvg) -> Self {
        Self { one, five, fifteen }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub total: u64,
    pub used: u64,
}

// TODO: ?????????
// /// Mapping of name->processes.
// type ProcessesView = HashMap<String, Vec<Process>>;
// /// Mapping of user->processes.
// type UsageView = HashMap<String, Vec<Process>>;

/// Per-process usage information for a single machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Processes(Box<[Process]>);

impl std::ops::Deref for Processes {
    type Target = [Process];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

type UsersView<'ps> = HashMap<&'ps str, Box<[&'ps Process]>>;

#[allow(dead_code)] // TODO: Fix this. When we know where we actually use this.
impl Processes {
    pub fn new(items: Box<[Process]>) -> Self {
        Self(items)
    }

    /// Returns a mapping of user names to the associated processes.
    pub fn by_users(&self) -> UsersView<'_> {
        let mut view = HashMap::<&str, Vec<_>>::new();
        for proc in self.iter() {
            view.entry(&proc.user).or_default().push(proc);
        }
        view.into_iter().map(|(name, procs)| (name, procs.into_boxed_slice())).collect()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Process {
    pub name: String,
    pub user: String,
    pub usage: f32,
}

impl Process {
    pub fn new(name: String, user: String, usage: f32) -> Self {
        Self { name, user, usage }
    }
}

pub struct CpuUsage {
    pub used: u32,
    pub total: u32,
}

pub struct ActiveUser {
    pub user: String,
    pub cores: u32,
    pub task: String,
}
