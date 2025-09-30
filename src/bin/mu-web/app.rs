use std::path::PathBuf;

use anyhow::{Context, Result};

use mu::info::{Data, HostInfo};

pub struct App {
    host_info: HostInfo,
    path: PathBuf,
    data: Option<Data>,
}

impl App {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        Ok(Self {
            host_info: HostInfo::new()?,
            path: path.as_ref().to_path_buf(),
            data: None,
        })
    }

    pub fn host_info(&self) -> &HostInfo {
        &self.host_info
    }

    /// Before reading, the data must be [refreshed](Self::refresh_data). If this is not the case,
    /// this function may return `None`.
    pub fn data(&self) -> Option<&Data> {
        self.data.as_ref()
    }

    pub fn refresh_data(&mut self) -> Result<&Data> {
        let data_path = &self.path;
        // TODO: Perhaps we can use a thread_local to re-use the allocation?

        // Read all usage data file contents at once in an attempt to avoid deserializing the file
        // contents while it is being written by `mu-hive`.
        let file = std::fs::read(data_path).context(format!(
            "could not open the path {data_path:?}, try providing a path as an argument"
        ))?;
        let data = serde_json::from_slice(&file)?;
        self.data = Some(data);
        Ok(self.data().unwrap())
    }
}
