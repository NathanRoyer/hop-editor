use crossterm::style::Color;
use hex_color::HexColor;
use serde::Deserialize;
use litemap::LiteMap;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Theme {
    #[serde(flatten)]
    inner: LiteMap<String, HexColor>,
}

impl Theme {
    pub fn parse(theme_str: &str) -> Result<Self, &'static str> {
        match toml::from_str(theme_str) {
            Ok(theme) => Ok(theme),
            Err(error) => {
                println!("toml_parse: {error:?}");
                Err("failed to parse theme file")
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<HexColor> {
        self.inner.get(key).copied()
    }

    pub fn get_ansi(&self, name: &str) -> Color {
        type Rgb = (u8, u8, u8);

        match self.get(name) {
            Some(hc) => Color::from(Rgb::from(hc)),
            None => Color::Reset,
        }
    }
}
