use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::str::FromStr;

use anyhow::Result;
use mu::info::LoadAvg;
use serde::Serialize;
use tera::{Context, Tera};

use crate::app::App;
use crate::data::DataView;

mod app;
mod data;

#[derive(Debug, Serialize)]
struct Data {
    machines: Box<[Machine]>,
}

impl<T: DataView> From<&T> for Data {
    fn from(value: &T) -> Self {
        Self {
            machines: value.machines(),
        }
    }
}

#[derive(Debug, Serialize)]
struct Machine {
    hostname: String,
    hotness: u32,
    owner: String,
    owner_mark: String,
    room: String,
    cpu_usage: CpuUsage,
    load_avg: LoadAvg,
    active_user: Option<ActiveUser>,
}

#[derive(Debug, Clone, Serialize)]
enum Owner {
    Member(String),
    Visitor(String),
    Student(String),
    Reserve,
    None,
}

impl FromStr for Owner {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
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

#[derive(Debug, Clone, Copy, Serialize)]
struct CpuUsage {
    used: u32,
    total: u32,
}

#[derive(Debug, Serialize)]
struct ActiveUser {
    user: String,
    cores: u32,
    task: String,
}

fn handle(mut stream: TcpStream, content: &str) -> Result<()> {
    let reader = BufReader::new(&stream);
    let _request = reader
        .lines()
        .map(|r| r.unwrap()) // TODO: Properly handle error.
        .take_while(|l| !l.is_empty())
        .collect::<Vec<_>>();
    // println!("Request: {request:?}");

    let status = "HTTP/1.1 200 OK";
    let length = content.len();
    let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n{content}");
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn main() -> Result<()> {
    let data_path = std::env::args()
        .skip(1)
        .next()
        .unwrap_or("/martini/sshuser/mu/mu.dat".to_string());

    let mut app = App::new(data_path)?;
    let data: Data = app.refresh_data()?.into();

    // Load the template.
    let template = Tera::new("templates/**/*")?;
    let template_names = template.get_template_names().collect::<Vec<_>>();
    eprintln!("INFO: Found templates with the following names: {template_names:?}");
    let context = Context::from_serialize(data)?;
    let content = template.render("index.html", &context)?;

    let listener = TcpListener::bind("127.0.0.1:5172")?;
    eprintln!("INFO: Listener set up.");
    for stream in listener.incoming() {
        eprintln!("INFO: Caught a stream! {stream:?}");
        match handle(stream?, &content) {
            Ok(_) => {}
            Err(err) => eprintln!("ERROR: {err}"),
        };
    }

    Ok(())
}
