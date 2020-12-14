#![feature(decl_macro)]

use radmin::rocket::http::Status;
use radmin::rocket::{Config, Route, State};
use radmin::modules::{ServerModule, RoutesModule, CliModule};
use radmin::{crate_name, crate_version, ServerError};

use std::env::var;
use std::path::PathBuf;
use radmin::rocket::response::NamedFile;

use std::io::Write;
use radmin::clap::{App, ArgMatches, SubCommand, AppSettings};
use std::fs::File;

mod bundler;

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
        let css_dir = var("CSS_DIR").unwrap_or_else(|_| "css".into());
        let asset_dir = var("ASSETS_DIR").unwrap_or_else(|_| "assets".into());
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
            ("assets".into(), radmin::rocket::routes![get_asset_file])
        ]
    }
}

#[radmin::rocket::get("/<file_name..>", rank = 2)]
fn get_asset_file(cfg: State<Config>, file_name: PathBuf) ->Result<NamedFile, Status> {
    let mut asset_dir: PathBuf = cfg.extras.get("assets_dir").unwrap().as_str().unwrap().into();
    asset_dir.push(file_name);
    NamedFile::open(&asset_dir).map_err(|_| Status::NotFound)
}


pub struct AssetsCliModule;

impl CliModule for AssetsCliModule {
    fn arg(&self) -> Option<String> {
        Some("assets".into())
    }

    fn app(&self) -> Option<App<'static, 'static>> {
        Some(SubCommand::with_name("assets")
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

fn handle_build() -> Result<(), ServerError > {
    build_styles()?;
    build_javascript()?;
    Ok(())
}

fn build_javascript() -> Result<(), ServerError> {
    let js_dir = var("JS_DIR").unwrap_or_else(|_| "js".into());
    let files = std::fs::read_dir(js_dir)?
            .map(|item|item.unwrap().path()).filter(|entry| {
        let file_name = entry.file_name().unwrap().to_os_string().into_string().unwrap();
        let metadata = entry.metadata().unwrap();
        if !file_name.starts_with("_") &&
           !file_name.starts_with(".") && metadata.is_file() {
            true
        } else {
            false
        }
    }).collect::<Vec<PathBuf>>();
    bundler::bundle(files)?;
    Ok(())
}

fn build_styles() -> Result<(), ServerError> {
    let css_dir = var("CSS_DIR").unwrap_or_else(|_| "css".into());
    let mut css_out_dir: PathBuf = var("ASSETS_DIR").unwrap_or_else(|_| "assets".into()).into();
    css_out_dir.push("css");
    std::fs::read_dir(css_dir)?.for_each(|_entry| {
        let entry = _entry.unwrap();
        let file_name = entry.file_name().into_string().unwrap();
        let metadata = entry.metadata().unwrap();
        if !file_name.starts_with("_") &&
           !file_name.starts_with(".") && metadata.is_file() {
            let options = sass_rs::Options {
                output_style: sass_rs::OutputStyle::Compressed,
                precision: 0,
                indented_syntax: false,
                include_paths: vec![]
            };
            let mut new_path = css_out_dir.clone();
            new_path.push(entry.path().file_name().unwrap());
            new_path.set_extension("css");
            match sass_rs::compile_file(&entry.path(), options) {
                Ok(data) => {
                    if new_path.exists() {
                        std::fs::remove_file(&new_path).unwrap();
                    }
                    if let Some(parent) = new_path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent).unwrap();
                        }
                    }
                    match File::create(&new_path) {
                        Err(err) => println!("Failed to open css file: {}", err),
                        Ok(mut file) => {
                            match file.write(data.as_ref()) {
                                Err(err) => println!("Failed to write to css file: {}", err),
                                Ok(_) => println!("Wrote Css File to: {:?}", new_path)
                            }
                        }
                    }
                },
                Err(err) => println!("Failed to compile css file: {}", err)
            }
        }
    });
    Ok(())
}
