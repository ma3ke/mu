use std::{io::Write, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use mu::info::{Data, RichInfo};
use openssh::{KnownHosts, Session};

use crate::config::{Machine, MachinesConfig};

mod config;

/// Coordinate the gathering of usage information for the network of machines.
///
/// The `hive` is responsible for reaching out to all machines specified in the machines
/// configuration and writing the collected usage information to a central data file. The
/// information in the data file is read by usage viewers and displayed.
///
/// The `bee` is executed on each machine specified in the machines configuration and gathers the
/// usage information for that machine. The information is serialized and sent back the connection
/// to the `hive`. In the `hive` the information gathered by the bees from all machines is
/// integrated and written to the output file.
#[derive(Debug, clap::Parser)]
struct Args {
    /// Path to an `.ini` formatted file listing all machines for each room.
    #[clap(long, short)]
    machines: PathBuf,
    /// Path for writing the collected output `.dat` file.
    #[clap(long, short)]
    output: PathBuf,
    /// Path to the `mu-bee` executable.
    ///
    /// The path should point to the location of the `mu-bee` executable from the perspective of
    /// each host machine listed in the machines configuration.
    #[clap(long, short)]
    bee: String,
}

pub async fn gather(machine: Machine, bee_path: &str) -> Result<RichInfo> {
    // TODO: Find out from openssh crate docs whether we want 'process-based' or 'mux-based' thing idk.
    let session = Session::connect(&machine.hostname, KnownHosts::Strict).await?;
    // TODO: See if it's possible to more directly stream the information to our deserializer.
    let hn = &machine.hostname;
    eprintln!("INFO: ({hn}) Connection established. Starting bee execution.");
    let bee = session.command(bee_path).output().await?;
    eprintln!("INFO: ({hn}) Executed bee. Deserializing.");
    let info =
        serde_json::from_slice(&bee.stdout).context("could not deserialize output from bee")?;
    eprintln!("INFO: ({hn}) Done.");
    Ok(RichInfo::new(info, machine.room, machine.note))
}

pub async fn peruse(machines_config: MachinesConfig, bee_path: &str) -> Result<Box<[RichInfo]>> {
    let tasks: Vec<_> = machines_config
        .into_iter()
        .cloned()
        .map(|machine| {
            let bee_path = bee_path.to_string();
            eprintln!("INFO: Setting up ssh into {:?}.", machine.hostname);
            tokio::spawn(async move {
                let hostname = machine.hostname.clone();
                gather(machine, &bee_path)
                    .await
                    .context(format!("problem while gathering usage from {hostname:?}"))
            })
        })
        .collect();

    let mut output_usage = Vec::new();
    for task in tasks {
        match task.await? {
            Ok(rich_info) => output_usage.push(rich_info),
            Err(e) => {
                let root_cause = e.root_cause();
                eprintln!("WARNING: {e}");
                eprintln!("         {root_cause}");
            }
        };
    }

    let nsuccess = output_usage.len();
    let n = machines_config.len();
    eprintln!("INFO: All machines have been perused. ({nsuccess}/{n} success)");
    Ok(output_usage.into_boxed_slice())
}

fn main() -> Result<()> {
    let start = std::time::Instant::now();
    let args = Args::parse();

    let machines_path = &args.machines;
    let machines_config = MachinesConfig::read_from_config(machines_path)
        .context(format!("could not process machines file {machines_path:?}"))?;

    let runtime = tokio::runtime::Runtime::new().context("could not set up async runtime")?;
    let info = runtime.block_on(async { peruse(machines_config, &args.bee).await })?;

    let data = Data::new(info);

    let output_path = &args.output;
    // We first serialize into memory before writing the file, rather than writing to the file
    // directly, to limit the time that the file is in an invalid state.
    let output = serde_json::to_string_pretty(&data).context(format!(
        "could not write collected usage to output file {output_path:?}"
    ))?;
    let mut output_file = std::fs::File::create(output_path)
        .context(format!("could not open output file {output_path:?}"))?;
    output_file.write_all(output.as_bytes())?;
    let timestamp = data.timestamp;
    eprintln!("INFO: Output was written to {output_path:?} with timestamp {timestamp}.");

    let duration = start.elapsed().as_secs_f32();
    eprintln!("INFO: Execution took {duration:.2} s.");

    Ok(())
}
