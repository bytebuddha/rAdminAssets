#![feature(decl_macro)]

use radmin::rocket::{Config, Route, State};
use radmin::rocket::config::Environment;
use radmin::modules::{ServerModule, RoutesModule, CliModule};
use radmin::{crate_name, crate_version, ServerError, ApiResponse};

use std::env::var;
use std::path::PathBuf;
use radmin::rocket::response::NamedFile;

use std::io::{Cursor, Write};
use rsass::output::Format;
use rocket::response::Responder;
use rocket::{Response, Request};
use rocket::http::{Status, ContentType};
use radmin::clap::{App, ArgMatches, SubCommand, AppSettings};
use std::fs::File;


#[derive(Default)]
pub struct AssetsModule;

impl ServerModule for AssetsModule {
    fn identifier(&self) -> &'static str {
        crate_name!()
    }

    fn version(&self) -> &'static str {
        crate_version!()
    }

    fn config(&self, mut config: Config) -> Config {
        let scss_dir = var("SCSS_DIR").unwrap_or_else(|_| "scss".into());
        let css_dir = var("CSS_DIR").unwrap_or_else(|_| "css".into());
        let asset_dir = var("ASSETS_DIR").unwrap_or_else(|_| "assets".into());
        config.extras.insert("scss_dir".into(), scss_dir.into());
        config.extras.insert("css_dir".into(), css_dir.into());
        config.extras.insert("assets_dir".into(), asset_dir.into());
        config
    }

    fn cli(&self) -> Box<dyn CliModule> {
        Box::new(AssetsCliModule)
    }

    fn routes(&self) -> Box<dyn RoutesModule> {
        Box::new(AssetsRoutesModule)
    }
}

pub struct AssetsRoutesModule;

impl RoutesModule for AssetsRoutesModule {
    fn routes(&self) -> Vec<(String, Vec<Route>)> {
        vec![
            ("assets".into(), radmin::rocket::routes![get_asset_file, get_css_file])
        ]
    }
}

#[radmin::rocket::get("/<file_name..>", rank = 2)]
fn get_asset_file(cfg: State<Config>, file_name: PathBuf) ->Result<NamedFile, ApiResponse> {
    let mut asset_dir: PathBuf = cfg.extras.get("assets_dir").unwrap().as_str().unwrap().into();
    asset_dir.push(file_name);
    NamedFile::open(&asset_dir).map_err(|_| ApiResponse::not_found())
}

#[radmin::rocket::get("/css/<file_name>", rank = 1)]
fn get_css_file<'r>(cfg: State<Config>, file_name: String) -> Result<AssetData, Status> {
    if let Environment::Development = cfg.environment {
        if let Ok(scss_dir) = cfg.get_extra("scss_dir") {
            let mut path = PathBuf::from(scss_dir.as_str().unwrap());
            path.push(file_name);
            path.set_extension("scss");
            let format = Format {
                style: rsass::output::Style::Introspection,
                precision: 10
            };
            match rsass::compile_scss_file(&path, format) {
                Ok(data) => Ok(AssetData::Data(data)),
                Err(_) => Err(Status::InternalServerError)
            }
        } else {
            Err(Status::NotFound)
        }
    } else {
        if let Ok(css_dir) = cfg.get_extra("css_dir") {
            let mut path = PathBuf::from(css_dir.as_str().unwrap());
            path.push(file_name);
            path.set_extension("css");
            Ok(AssetData::File(path))
        } else {
            Err(Status::NotFound)
        }
    }
}

pub enum AssetData {
    Data(Vec<u8>),
    File(PathBuf)
}

impl<'r> Responder<'r> for AssetData {
    fn respond_to(self, req: &Request) -> rocket::response::Result<'r> {
        match self {
            AssetData::Data(data) => {
                Ok(Response::build()
                    .sized_body(Cursor::new(data))
                    .header(ContentType::new("text", "css"))
                    .finalize())
            },
            AssetData::File(file) => {
                NamedFile::open(file).respond_to(req)
            }
        }
    }
}

pub struct AssetsCliModule;

impl CliModule for AssetsCliModule {
    fn arg(&self) -> Option<String> {
        Some("assets".into())
    }

    fn app(&self) -> Option<App<'static, 'static>> {
        Some(SubCommand::with_name("css")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("build"))
        )
    }

    fn handle<'a>(&self, matches: Option<&ArgMatches<'a>>) -> Result<(), ServerError> {
        match matches.unwrap().subcommand() {
            ("build", _) => handle_build(),
            _ => unreachable!()
        }
    }
}

fn handle_build() -> Result<(), ServerError> {
    let scss_dir = var("SCSS_DIR").unwrap_or_else(|_| "scss".into());
    let css_dir: PathBuf = var("CSS_DIR").unwrap_or_else(|_| "css".into()).into();
    std::fs::read_dir(scss_dir)?.for_each(|_entry| {
        let entry = _entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        if !file_name.starts_with("_") &&
           !file_name.starts_with(".") &&
           !entry.metadata().unwrap().is_dir() {
           let format = Format {
                style: rsass::output::Style::Compressed,
                precision: 10
            };
            let mut new_path = css_dir.clone();
            new_path.push(entry.path().file_name().unwrap());
            match rsass::compile_scss_file(&entry.path(), format) {
                Ok(data) => {
                    new_path.set_extension("css");
                    if new_path.exists() {
                        std::fs::remove_file(&new_path).unwrap();
                    }
                    match File::create(&new_path) {
                        Err(err) => println!("Failed to open css file: {:?}", err),
                        Ok(mut file) => {
                            match file.write(data.as_ref()) {
                                Err(err) => println!("Failed to write to css file: {:?}", err),
                                Ok(_) => {}
                            }
                        }
                    }
                    println!("Wrote Css File to: {:?}", new_path);
                },
                Err(err) => println!("Failed to compile scss file: {:?}", err)
            }
        }
    });
    Ok(())
}