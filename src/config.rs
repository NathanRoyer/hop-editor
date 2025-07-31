use crate::syntax::SyntaxFile;
use crossterm::style::Color;
use std::sync::OnceLock;
use hex_color::HexColor;
use serde::Deserialize;
use litemap::LiteMap;
use crate::confirm;
use std::{fs, env};

type Rgb = (u8, u8, u8);

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Config {
    syntax: LiteMap<String, HexColor>,
    general: General,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct General {
    background: Option<HexColor>,
    syntax_file: Option<String>,
    hover: HexColor,
    tree_width: u16,
}

fn read_toml<'a>(
    default_value: &'static str,
    custom: bool,
    path: &str,
    dst: &'a mut String,
) -> &'a str {
    match fs::read_to_string(path).map(|s| *dst = s) {
        Err(e) if custom => panic!("failed to load config file: {e:?}"),
        Err(_) => default_value,
        Ok(()) => dst.as_str(),
    }
}

fn load() -> Config {
    let path = env::var("HOP_CONFIG");
    let mut tmp = String::new();

    let (custom, path) = match path.as_ref() {
        Ok(path) => (true, path.as_str()),
        Err(_) => (false, "~/.config/hop/config.toml"),
    };

    let config_str = read_toml(
        crate::DEFAULT_CONFIG,
        custom,
        path,
        &mut tmp,
    );

    match toml::from_str(&config_str) {
        Ok(theme) => theme,
        Err(error) => {
            confirm!("failed to parse config: {:#?}", error.message());
            let failure = "failed to parse fallback config file";
            toml::from_str(crate::DEFAULT_CONFIG).expect(failure)
        },
    }
}

fn color(opt: Option<&HexColor>) -> Color {
    match opt {
        Some(hc) => Color::from(Rgb::from(*hc)),
        None => Color::Reset,
    }
}

fn config() -> &'static Config {
    CONFIG.get_or_init(load)
}

pub fn init() {
    config();
}

pub fn tree_width() -> u16 {
    config().general.tree_width
}

pub fn syntax_file() -> SyntaxFile {
    let default = (false, "~/.config/hop/syntax.toml");
    let mut tmp = String::new();

    let (custom, path) = config()
        .general
        .syntax_file
        .as_ref()
        .map(|p| (true, p.as_str()))
        .unwrap_or(default);

    let syntax_str = read_toml(
        crate::DEFAULT_SYNTAX,
        custom,
        path,
        &mut tmp,
    );

    SyntaxFile::parse(syntax_str).unwrap_or_default()
}

pub fn default_bg_color() -> Color {
    color(config().general.background.as_ref())
}

pub fn hover_color() -> Color {
    color(Some(&config().general.hover))
}

pub fn ansi_color(name: &str) -> Color {
    color(config().syntax.get(name))
}
