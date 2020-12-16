use std::path::PathBuf;
use std::collections::HashMap;
use radmin::ServerError;
use swc_bundler::{Bundler, Config, Hook, Load, ModuleData, ModuleRecord, Resolve};
use swc_common::{sync::Lrc, FileName, FilePathMapping, Globals, SourceMap, Span};
use swc_ecma_ast::KeyValueProp;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter};
use swc_ecma_parser::{lexer::Lexer, EsConfig, Parser, StringInput, Syntax};
use anyhow::Error;
use termion::{ color, style };

struct Noop;

impl Hook for Noop {
    fn get_import_meta_props(&self, _: Span, _: &ModuleRecord) -> Result<Vec<KeyValueProp>, Error> {
        unimplemented!()
    }
}

struct PathResolver;

impl Resolve for PathResolver {
    fn resolve(&self, base: &FileName, module_specifier: &str) -> Result<FileName, Error> {
        let base = match base {
            FileName::Real(v) => v,
            _ => unreachable!(),
        };

        Ok(FileName::Real(
            base.parent()
                .unwrap()
                .join(module_specifier)
                .with_extension("js"),
        ))
    }
}

struct PathLoader {
    cm: Lrc<SourceMap>,
}

impl Load for PathLoader {
    fn load(&self, file: &FileName) -> Result<ModuleData, Error> {
        let file = match file {
            FileName::Real(v) => v,
            _ => unreachable!(),
        };

        let fm = self.cm.load_file(file)?;
        let lexer = Lexer::new(
            Syntax::Es(EsConfig {
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("This should not happen");

        Ok(ModuleData {
            fm,
            module,
            helpers: Default::default(),
        })
    }
}

pub fn bundle(files: Vec<PathBuf>) -> Result<(), ServerError> {
    let globals = Globals::new();
    let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
    let external_modules = vec![];
    let bundler = Bundler::new(
        &globals,
        cm.clone(),
        PathLoader { cm: cm.clone() },
        PathResolver,
        Config {
            require: true,
            external_modules,
            ..Default::default()
        },
        Box::new(Noop)
    );

    let js_out_dir: PathBuf = std::env::var("ASSETS_DIR").unwrap_or_else(|_| "assets".into()).into();

    println!("{}Writing JS files{}:", color::Fg(color::Green), color::Fg(color::Reset));

    for file in files {
        let mut entries = HashMap::default();
        entries.insert("main".to_string(), FileName::Real(file.clone().into()));

        let mut bundles = bundler.bundle(entries).expect("Failed to create file bundle");
        let bundle = bundles.pop().unwrap();
        let mut out_file = js_out_dir.clone();
        out_file.push(&file);

        if out_file.exists() {
            std::fs::remove_file(&out_file)?;
        }
        if let Some(parent) = out_file.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let writer = std::fs::File::create(&out_file)?;
        let mut emitter = Emitter {
            cfg: swc_ecma_codegen::Config { minify: false },
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(cm.clone(), "\n", writer, None)),
        };

        emitter.emit_module(&bundle.module)?;
        println!(
            "    {}✓{} {}{:?}{} {}->{} {}{:?}{}",
            color::Fg(color::Green),
            color::Fg(color::Reset),
            style::Italic,
            file,
            style::Reset,
            color::Fg(color::Cyan),
            color::Fg(color::Reset),
            style::Italic,
            out_file,
            style::Reset
        );
    }
    Ok(())
}
