use mu::model::{Memory, PROCESS_USAGE_THRESHOLD_PERCENT, Process, Processes, Usage};

use crate::config::Config;

// TODO: Consider name space polution with `gather` function in mu-hive.
pub trait Gather {
    fn gather(system: &sysinfo::System, config: Config) -> Self;
}

impl Gather for Usage {
    fn gather(system: &sysinfo::System, config: Config) -> Self {
        // TODO: Consider if this value is meaningfully different here than if we request it
        // _right_ after initializing the System, when the load average has been minimally poisoned
        // by our presence.

        // Request the load average of the system before doing much processing ourselves.
        let load_avg = sysinfo::System::load_average().into();

        let mut procs = Vec::new();
        let users = sysinfo::Users::new_with_refreshed_list();
        for proc in system.processes().values() {
            // Ignore the process of this program.
            if sysinfo::get_current_pid().is_ok_and(|pid| pid == proc.pid()) {
                continue;
            }

            let mut name = proc.name().to_string_lossy().to_string();
            let user = proc
                .effective_user_id()
                .or(proc.user_id())
                .and_then(|uid| users.get_user_by_id(uid))
                .map(|u| u.name())
                .unwrap_or("?")
                .to_string();
            let cpu_usage = proc.cpu_usage();

            // Ignore processes based on their name, user, or due to low usage values.
            let ignore = config.is_ignored_user(&user) || config.is_ignored_process(&name);
            let low_usage = cpu_usage < PROCESS_USAGE_THRESHOLD_PERCENT;
            if ignore || low_usage {
                continue;
            }

            // Rename if necessary.
            if let Some(renamed) = config.get_canonical_name(&name) {
                name = renamed.to_string();
            }

            procs.push(Process::new(name.clone(), user.clone(), cpu_usage));
        }

        Self {
            global_cpu_usage: system.global_cpu_usage(),
            cpus: system.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
            load_avg,
            mem: Memory { total: system.total_memory(), used: system.used_memory() },
            processes: Processes::new(procs.into_boxed_slice()),
        }
    }
}
