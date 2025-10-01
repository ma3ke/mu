use std::str::FromStr;

use actix_web::{HttpResponse, HttpServer, Responder, get, web};
use anyhow::Result;
use mu::info::LoadAvg;
use serde::Serialize;
use tera::Tera;

use crate::app::State;
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

#[get("/")]
async fn base(data: web::Data<State>) -> impl Responder {
    let content = data.render("index.html").unwrap();
    HttpResponse::Ok().body(content)
}

#[get("/machines")]
async fn machines(data: web::Data<State>) -> impl Responder {
    let content = data.render("machines.html").unwrap();
    HttpResponse::Ok().body(content)
}

#[tokio::main]
async fn main() -> Result<()> {
    let data_path = std::env::args()
        .skip(1)
        .next()
        .unwrap_or("/martini/sshuser/mu/mu.dat".to_string());

    // Load the template.
    let templates = Tera::new("templates/**/*")?;
    let template_names = templates.get_template_names().collect::<Vec<_>>();
    eprintln!("INFO: Found templates with the following names: {template_names:?}");

    let mut state = State::new(data_path, templates)?;
    state.refresh_data()?;

    HttpServer::new(move || {
        actix_web::App::new()
            .app_data(web::Data::new(state.clone()))
            .service(base)
            .service(machines)
    })
    .bind("0.0.0.0:5172")?
    .run()
    .await?;
    Ok(())
}
