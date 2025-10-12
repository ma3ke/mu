use std::collections::HashMap;

use mu::model::{
    ActiveUser, ClusterData, ClusterUsage, CpuUsage, HostInfo, LoadAvg, MachineDefinition,
    MachineUsage, Owner, PROCESS_USAGE_THRESHOLD_PERCENT, Usage,
};

pub struct ClusterDataView {
    pub header: HeaderView,
    pub stats: StatsView,
    pub notes: NotesView,
    pub machines: Box<[MachineView]>,
}

impl ClusterDataView {
    // TODO: Remove the logged thing.
    pub fn new(
        hostinfo: HostInfo,
        data: &ClusterData,
        logged: bool,
        success: bool,
        show_room: bool,
    ) -> Self {
        let header = HeaderView::new(hostinfo, &data.usage);
        let stats = StatsView::new(&data.usage);
        let notes = NotesView::new(&data, logged, success);
        let mut machines = data
            .usage
            .iter()
            .map(|machine| MachineView::new(machine, show_room))
            .collect::<Box<[_]>>();
        machines.sort_by_key(|machine| machine.hostname.clone());
        Self { header, stats, notes, machines }
    }
}

pub struct HeaderView {
    pub hostinfo: HostInfo,
    pub total_usage: f32,
}

impl HeaderView {
    pub fn new(hostinfo: HostInfo, usage: &ClusterUsage) -> Self {
        let total_cores_used: f32 =
            usage.iter().map(|entry| entry.usage.cpus.iter().sum::<f32>()).sum();
        let total_cores: f32 =
            usage.iter().map(|entry| entry.usage.cpus.len() as f32 * 100.0).sum();
        Self { hostinfo, total_usage: total_cores_used / total_cores }
    }
}

/// A list of `(user, usage_percent)` pairs.
pub struct StatsView(Box<[(String, f32)]>);

impl std::ops::Deref for StatsView {
    type Target = [(String, f32)];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StatsView {
    pub fn new<'a>(usage: &'a ClusterUsage) -> Self {
        // Create a list of `(user, total_threads)` pairs.
        let mut tpu = HashMap::<_, usize>::new();
        for machine in usage.iter() {
            for (user, procs) in machine.usage.processes.by_users() {
                *tpu.entry(user).or_default() += procs.len();
            }
        }

        // Note that we place the number of threads before the user name, so that the entries are
        // sorted based on thread count first, and then by the user name to break ties.
        let mut tpu = tpu.into_iter().map(|(user, threads)| (threads, user)).collect::<Vec<_>>();
        tpu.sort();

        let total_cpus = usage.cpu_count() as f32;
        let stats = tpu
            .into_iter()
            .rev()
            .filter_map(|(threads, user)| {
                if threads == 0 {
                    return None;
                }
                let usage_percent = 100.0 * threads as f32 / total_cpus;
                if usage_percent < 1.0 {
                    return None;
                }
                Some((user.to_owned(), usage_percent))
            })
            .collect();
        Self(stats)
    }
}

pub struct NotesView {
    pub last_update: std::time::SystemTime,
    pub logged: bool,
    pub success: bool,
}

impl NotesView {
    fn new(data: &ClusterData, logged: bool, success: bool) -> Self {
        Self { last_update: data.time(), logged, success }
    }
}

pub struct MachineView {
    pub hostname: String,
    pub owner: Owner,
    pub room: String,
    pub cpu_usage: CpuUsage,
    pub load_avg: LoadAvg,
    pub active_user: Option<ActiveUser>,
    pub show_room: bool,
}

impl MachineView {
    pub fn new(machine: &MachineUsage, show_room: bool) -> Self {
        // TODO: Consider doing the whole lifetime thing here.
        let MachineDefinition { hostname, owner, room } = machine.definition.clone();
        let Usage { global_cpu_usage: _, cpus, load_avg, mem: _, processes } =
            machine.usage.clone();
        let cpu_usage = CpuUsage {
            used: cpus.iter().filter(|&&u| u > PROCESS_USAGE_THRESHOLD_PERCENT).count() as u32,
            total: cpus.len() as u32,
        };
        let active_user = processes
            .by_users()
            .into_iter()
            .max_by_key(|(_, cores)| cores.iter().map(|cu| cu.usage as u64).sum::<u64>())
            .map(|(user, procs)| ActiveUser {
                user: user.to_string(),
                cores: procs.len() as u32,
                task: procs
                    .iter()
                    .max_by_key(|proc| proc.usage as u64)
                    .map(|cu| cu.name.to_string())
                    .unwrap_or("?".to_string()),
            });
        Self { hostname, owner, room, cpu_usage, load_avg, active_user, show_room }
    }
}
