use std::collections::HashMap;

/// The structure stored in `machine_usage.dat`
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Data {
    pub timestamp: u64,
    pub info: Box<[RichInfo]>,
}

/// Information for a single machine associated with room and an owner note.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
// TODO: Name.
pub struct RichInfo {
    pub room: String,
    pub note: Option<String>,
    pub info: Info,
}

impl RichInfo {
    pub fn new(info: Info, room: String, note: Option<String>) -> Self {
        Self { room, note, info }
    }
}

/// Information for a single machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Info {
    pub hostname: String,
    pub global_cpu_usage: f32,
    pub cpus: Box<[f32]>,
    pub load_avg: LoadAvg,
    pub mem: Memory,
    // The `processes` and `usage` things can be merged into a collection of `Process` structs with
    // extra information that we just iterate over in different ways to get at different
    // relationships.
    pub processes: Processes,
    pub usage: Usage,
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

/// Mapping of name->processes.
type Processes = HashMap<String, Vec<Process>>;
/// Mapping of user->processes.
type Usage = HashMap<String, Vec<Process>>;

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

impl Info {
    pub fn new(system: &sysinfo::System) -> Self {
        // TODO: Consider if this value is meaningfully different here than if we request it
        // _right_ after initializing the System, when the load average has been minimally poisoned
        // by our presence.

        // Request the load average of the system before doing much processing ourselves.
        let load_avg = sysinfo::System::load_average().into();

        // TODO: Reconsider whether the ? defaulting is good or not.
        let hostname = hostname::get()
            .ok()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or("?".to_string());

        let mut usage = Usage::new();
        let mut processes = Processes::new();
        let users = sysinfo::Users::new_with_refreshed_list();
        for proc in system.processes().values() {
            // Ignore the process of this program.
            if sysinfo::get_current_pid().is_ok_and(|pid| pid == proc.pid()) {
                continue;
            }

            let name = proc.name().to_string_lossy().to_string();
            let user = proc
                .effective_user_id()
                .or(proc.user_id())
                .and_then(|uid| users.get_user_by_id(uid))
                .map(|u| u.name())
                .unwrap_or("?")
                .to_string();
            let cpu_usage = proc.cpu_usage();

            // Ignore processes with low usage values.
            const PROCESS_USAGE_THRESHOLD_PERCENT: f32 = 5.0;
            if cpu_usage < PROCESS_USAGE_THRESHOLD_PERCENT {
                continue;
            }

            let proc = Process::new(name.clone(), user.clone(), cpu_usage);
            processes.entry(name).or_default().push(proc.clone());
            usage.entry(user).or_default().push(proc);
        }

        Self {
            hostname,
            global_cpu_usage: system.global_cpu_usage(),
            cpus: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
            load_avg,
            mem: Memory {
                total: system.total_memory(),
                used: system.used_memory(),
            },
            usage,
            processes,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub total: u64,
    pub used: u64,
}
