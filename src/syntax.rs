use serde::Deserialize;
use litemap::LiteMap;
use RangeMode::*;

/* CONFIG STRUCT */

#[derive(Copy, Clone, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum NumberType {
    Dec,
    Hex,
    Bin,
    Oct,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct StringConfig {
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
pub struct SyntaxConfig {
    strings_normal: Vec<StringConfig>,

    #[serde(default)]
    strings_special: Vec<StringConfig>,

    #[serde(default)]
    multi_line_comments: Vec<StringConfig>,

    comment_prefix: Vec<String>,
    keywords_strong: Vec<String>,
    keywords_basic: Vec<String>,
    keywords_weak: Vec<String>,
    number_glue: Vec<char>,
    numbers: Vec<NumberType>,
    call_syms: Vec<String>,
    symbols: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct SyntaxFile {
    #[serde(flatten)]
    inner: LiteMap<String, SyntaxConfig>,
}

impl SyntaxFile {
    pub fn parse(config_str: &str) -> Result<Self, &'static str> {
        match toml::from_str(config_str) {
            Ok(config) => Ok(config),
            Err(error) => {
                println!("toml_parse: {error:?}");
                Err("failed to parse syntax file")
            }
        }
    }

    pub fn get(&self, syntax_name: &str) -> Option<&SyntaxConfig> {
        self.inner.get(syntax_name)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Casing {
    Upper,
    Lower,
    Mixed,
}

impl Casing {
    pub fn detect(identifier: &str) -> Self {
        let has_lower = identifier.chars().any(|c| c.is_lowercase());
        let has_upper = identifier.chars().any(|c| c.is_uppercase());

        match (has_lower, has_upper) {
            (false, false) => Casing::Lower,
            (true, false) => Casing::Lower,
            (false, true) => Casing::Upper,
            (true, true) => Casing::Mixed,
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum RangeMode {
    Identifier(Casing),
    StringNormal,
    StringSpecial,
    Comment,
    StringEscape,
    StringFormat,
    Symbol,
    Number(NumberType),
    KeywordStrong,
    KeywordBasic,
    KeywordWeak,
    Call(Casing),
    #[default]
    Whitespace,
}

impl RangeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Identifier(Casing::Mixed) => "imixed",
            Identifier(Casing::Lower) => "ilower",
            Identifier(Casing::Upper) => "iupper",
            KeywordStrong => "kw-strong",
            KeywordBasic => "kw-basic",
            KeywordWeak => "kw-weak",
            StringSpecial => "spe-str",
            StringNormal => "string",
            StringFormat => "format",
            StringEscape => "escape",
            Comment => "comment",
            Symbol => "symbol",
            Whitespace => "wspace",
            Number(NumberType::Hex) => "numhex",
            Number(NumberType::Dec) => "numdec",
            Number(NumberType::Bin) => "numbin",
            Number(NumberType::Oct) => "numoct",
            Call(Casing::Mixed) => "cmixed",
            Call(Casing::Lower) => "clower",
            Call(Casing::Upper) => "cupper",
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Range {
    pub len: usize,
    pub mode: RangeMode,
}

struct State<'a> {
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
    pub fn highlight(&self, target: &str) -> Vec<Range> {
        let mut state = State::new(self, target);

        state.process(SyntaxConfig::find_comments_and_strings);
        state.process(SyntaxConfig::find_symbols);
        state.process(SyntaxConfig::find_whitespace);
        state.process(SyntaxConfig::find_others);

        // tag function calls
        let (mut i, mut j) = (0, 1);

        while j < state.ranges.len() {
            let (a_start, a_range) = state.ranges.get_indexed(i).unwrap();
            let (b_start, b_range) = state.ranges.get_indexed(j).unwrap();

            if let Identifier(casing) = a_range.mode {
                let snippet = &state.target[*b_start..][..b_range.len];

                if self.call_syms.iter().any(|sym| sym == snippet) {
                    let key = *a_start;
                    state.ranges[&key].mode = Call(casing);
                }
            }

            i = j;
            j += 1;
        }

        // todo: tag string formats and escapes

        let vec = state.ranges.into_tuple_vec();
        vec.into_iter().map(|(_, r)| r).collect()
    }

    fn find_comments_and_strings(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let mut candidate = None;

        for symbol in &self.comment_prefix {
            let mut search_area = &target[start..stop];
            let mode = Comment;

            if let Some(offset) = search_area.find(symbol) {
                search_area = &search_area[offset + symbol.len()..];
                let maybe_len = search_area.find('\n');
                let len = maybe_len.unwrap_or(search_area.len()) + symbol.len();
                candidate = Some((start + offset, Range { mode, len }));
            }
        }

        let string_specs = [
            (&self.multi_line_comments, Comment),
            (&self.strings_normal, StringNormal),
            (&self.strings_special, StringSpecial),
        ];

        for (configs, mode) in string_specs {
            for string_cfg in configs {
                let Some((r_start, r_range)) = string_cfg.find(target, start, stop, mode) else {
                    continue;
                };

                let replace = match candidate {
                    None => true,
                    Some((c_start, _)) => r_start < c_start,
                };

                if replace {
                    candidate = Some((r_start, r_range));
                }
            }
        }

        candidate
    }
}

impl SyntaxConfig {
    fn find_symbols(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let mode = Symbol;

        for symbol in &self.symbols {
            if let Some(offset) = target[start..stop].find(symbol) {
                let range = Range { mode, len: symbol.len() };
                return Some((start + offset, range));
            }
        }

        None
    }

    fn find_whitespace(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let mode = Whitespace;
        let mut started = false;
        let mut offset = 0;
        let mut length = 0;

        for c in target[start..stop].chars() {
            let valid = c.is_whitespace();

            let len_target = match valid {
                true => &mut length,
                false => &mut offset,
            };

            if started & !valid {
                break;
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

    fn find_others(&self, target: &str, start: usize, stop: usize) -> Option<KeyedRange> {
        let area = &target[start..stop];

        if area.len() == 0 {
            return None;
        }

        let mode = match self.classify_number(area) {
            Some(num_type) => Number(num_type),
            None => match self.classify_keyword(area) {
                Some(mode) => mode,
                None => Identifier(Casing::detect(area)),
            },
        };

        let range = Range { mode, len: area.len() };
        return Some((start, range));
    }

    fn classify_number(&self, area: &str) -> Option<NumberType> {
        // todo: floats

        let number_classes = [
            ( "" , 10, NumberType::Dec),
            ("0x", 16, NumberType::Hex),
            ("0b",  2, NumberType::Bin),
            ("0o",  8, NumberType::Oct),
        ];

        for (prefix, radix, num_type) in number_classes {
            if self.numbers.contains(&num_type) {
                let Some(number_str) = area.strip_prefix(prefix) else {
                    break;
                };

                let is_glue = |c: char| self.number_glue.contains(&c);
                let valid_c = |c: char| c.is_digit(radix) || is_glue(c);

                if number_str.chars().all(valid_c) {
                    return Some(num_type);
                }
            }
        }

        None
    }

    fn classify_keyword(&self, identifier: &str) -> Option<RangeMode> {
        let keyword_classes = [
            (&self.keywords_strong, KeywordStrong),
            (&self.keywords_basic, KeywordBasic),
            (&self.keywords_weak, KeywordWeak),
        ];

        for (keywords, mode) in keyword_classes {
            if keywords.iter().any(|kw| kw == identifier) {
                return Some(mode);
            }
        }

        None
    }
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
