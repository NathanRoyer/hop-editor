
use serde::Deserialize;
use litemap::LiteMap;

/* CONSTANTS */


/* CONFIG STRUCT */

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
enum NumberType {
    Dec,
    Hex,
    Bin,
    Oct,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct StringConfig {
    start: String,
    stop: String,
    escape: Vec<char>,

    #[serde(default)]
    strfmt_percent: bool,

    #[serde(default)]
    strfmt_braces: bool,

    #[serde(default)]
    single_char: bool,

    #[serde(default)]
    multi_line: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct SyntaxConfig {
    strings_normal: Vec<StringConfig>,

    #[serde(default)]
    strings_special: Vec<StringConfig>,

    #[serde(default)]
    multi_line_comments: Vec<StringConfig>,

    comment_prefix: Vec<String>,
    keywords_strong: Vec<String>,
    keywords_basic: Vec<String>,
    keywords_weak: Vec<String>,
    identifier_glue: Vec<char>,
    number_glue: Vec<char>,
    numbers: Vec<NumberType>,
    call_syms: Vec<char>,
    symbols: Vec<String>,
}

type SyntaxFile = LiteMap<String, SyntaxConfig>;


/* ---- GENERIC UTILITIES ---- */

fn toml_parse(config_str: &str) -> Result<SyntaxFile, &'static str> {
    match toml::from_str(config_str) {
        Ok(config) => Ok(config),
        Err(error) => {
            println!("toml_parse: {error:?}");
            Err("failed to parse syntax file")
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Casing {
    Uppercase,
    Lowercase,
    Mixed,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RangeMode {
    // upper, lower, mixed
    Identifier(Casing),
    // string
    StringNormal,
    // ?
    StringSpecial,
    // comment
    Comment,
    // escape
    StringEscape,
    // format
    StringFormat,
    // symbol
    Symbol,
    // number
    Number,
    // ?
    KeywordStrong,
    KeywordBasic,
    KeywordWeak,
    // fncall
    Call,
}

#[derive(Copy, Clone, Debug)]
pub struct Range {
    len: usize,
    mode: RangeMode,
}

#[derive(Debug)]
pub struct State<'a> {
    ranges: LiteMap<usize, Range>,
    syntax: &'a SyntaxConfig,
    target: &'a str,
}

type KeyedRange = (usize, Range);

impl<'a> State<'a> {
    fn new(syntax: &'a SyntaxConfig, target: &'a str) -> Self {
        Self {
            ranges: Default::default(),
            syntax,
            target,
        }
    }

    fn process<F: Fn(&SyntaxConfig, &str, usize, usize) -> Option<KeyedRange>>(&mut self, func: F) {
        let mut cursor = 0;

        let mut i = 0;

        while cursor < self.target.len() {
            // locate next range
            let (nr_start, nr_len, new_i) = match self.ranges.get_indexed(i) {
                Some((range_start, range)) => (*range_start, range.len, i + 1),
                None => (self.target.len(), 0, i),
            };

            if let Some((offset, new_range)) = func(&self.syntax, self.target, cursor, nr_start) {
                // repeat the search before this new token
                // (with i unchanged, we will now scan from cursor to offset)
                self.ranges.insert(offset, new_range);
            } else {
                // nothing to see here; move along
                cursor = nr_start + nr_len;
                i = new_i;
            }
        }
    }
}

impl SyntaxConfig {
    fn find_comments_and_strings(&self, target: &str, mut start: usize, stop: usize) -> Option<KeyedRange> {
        for symbol in &self.comment_prefix {
            let mut search_area = &target[start..stop];
            let mode = RangeMode::Comment;

            if let Some(offset) = search_area.find(symbol) {
                search_area = &search_area[offset + symbol.len()..];
                let maybe_len = search_area.find('\n');
                let len = maybe_len.unwrap_or(search_area.len()) + symbol.len();
                return Some((start + offset, Range { mode, len }));
            }
        }

        let string_specs = [
            (&self.multi_line_comments, RangeMode::Comment),
            (&self.strings_normal, RangeMode::StringNormal),
            (&self.strings_special, RangeMode::StringSpecial),
        ];

        for (configs, mode) in string_specs {
            for string_cfg in configs {
                if let Some(ret) = string_cfg.find(target, start, stop, mode) {
                    return Some(ret);
                }
            }
        }

        None
    }
}

impl SyntaxConfig {
    fn find_symbols(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let mode = RangeMode::Symbol;

        for symbol in &self.symbols {
            if let Some(offset) = target[start..stop].find(symbol) {
                let range = Range { mode, len: symbol.len() };
                return Some((start + offset, range));
            }
        }

        None
    }
}

impl SyntaxConfig {
    fn find_identifiers(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let mode = RangeMode::Identifier(Casing::Mixed);

        let mut started = false;
        let mut offset = 0;
        let mut length = 0;

        for c in target[start..stop].chars() {
            let valid = match started {
                false => c.is_alphabetic(),
                true if c.is_alphanumeric() => true,
                true => self.identifier_glue.contains(&c),
            };

            let len_target = match valid {
                true => &mut length,
                false => &mut offset,
            };

            if started & !valid {
                let range = Range { mode, len: length };
                return Some((start + offset, range));
            }

            started |= valid;
            *len_target += c.len_utf8();
        }

        if started {
            let range = Range { mode, len: length };
            return Some((start + offset, range));
        } else {
            None
        }
    }
}

pub fn parse_file() -> Result<(), &'static str> {
    let rust_src = include_str!("syntax.rs");

    let syntax_cfg_str = include_str!("syntax.toml");
    let syntaxes = toml_parse(syntax_cfg_str)?;

    let Some(syntax) = syntaxes.get("rust") else {
        return Err("syntax not found");
    };

    let mut state = State::new(syntax, rust_src);

    state.process(SyntaxConfig::find_comments_and_strings);
    state.process(SyntaxConfig::find_symbols);
    state.process(SyntaxConfig::find_identifiers);

    // - sort identifiers
    // - find function calls
    // - string format

    for (start, range) in state.ranges {
        match range.mode {
            RangeMode::StringNormal => println!("string @ {start}: {}", &state.target[start..][..range.len]),
            RangeMode::Symbol => println!("symbol @ {start}: {}", &state.target[start..][..range.len]),
            RangeMode::Identifier(_) => println!("ident @ {start}: {}", &state.target[start..][..range.len]),
            RangeMode::Comment => println!("comment @ {start}: {}", &state.target[start..][..range.len]),
            _other => (),
        }
    }

    Ok(())
}

impl StringConfig {
    fn find(&self, target: &str, mut start: usize, stop: usize, mode: RangeMode) -> Option<KeyedRange> {
        loop {
            let mut search_area = &target[start..stop];
            let string_start = start + search_area.find(&self.start)?;

            let mut skip = string_start + self.start.len();
            search_area = &target[skip..stop];

            // for single-char rules only
            let mut esc_len = 1;

            // detect escaped characters
            while let Some((before, after)) = search_area.split_once('\\') {
                if before.contains(&self.stop) {
                    break;
                }

                skip += before.len() + 1;

                for allowed_esc in &self.escape {
                    if after.starts_with(*allowed_esc) {
                        esc_len = 2;
                        skip += allowed_esc.len_utf8();
                    }
                }

                // todo: handle unallowed escape
                search_area = &target[skip..stop];
            }

            let Some(stop_index) = search_area.find(&self.stop) else {
                start = string_start + self.start.len();
                continue;
            };

            let string_stop = skip + stop_index + self.stop.len();

            let len = string_stop
                .checked_sub(string_start)
                .expect("unexpected underflow");

            search_area = &target[string_start..string_stop];
            if !self.multi_line && search_area.contains('\n') {
                start = string_start + self.start.len();
                continue;
            }

            let max_len = || self.start.len() + self.stop.len() + esc_len;

            if self.single_char && len > max_len() {
                start = string_start + self.start.len();
                continue;
            }

            break Some((string_start, Range { mode, len }));
        }
    }
}
