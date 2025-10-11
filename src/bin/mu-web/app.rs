use std::path::PathBuf;

use anyhow::{Context, Result};

use mu::model::Data;
use tera::Tera;

#[derive(Debug, Clone)]
pub struct State {
    path: PathBuf,
    data: Option<Data>,
    templates: Tera,
}

impl State {
    pub fn new<P: AsRef<std::path::Path>>(path: P, templates: Tera) -> Result<Self> {
        Ok(Self { path: path.as_ref().to_path_buf(), data: None, templates })
    }

    pub fn render(&self, template_name: &str) -> Result<String> {
        let data = self.data().expect("data must have been refreshed before");
        let data = crate::Data::from(data);
        let context = tera::Context::from_serialize(data)?;
        let content = self.templates.render(template_name, &context)?;
        Ok(content)
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
