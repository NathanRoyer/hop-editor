use serde::Deserialize;
use litemap::LiteMap;
use std::sync::Arc;
use std::mem::take;
use crate::alert;
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
    extension: String,
    strings_normal: Vec<StringConfig>,

    #[serde(default)]
    strings_special: Vec<StringConfig>,

    #[serde(default)]
    multi_line_comments: Vec<StringConfig>,

    #[serde(default)]
    remap: LiteMap<String, String>,

    comment_prefix: Vec<String>,
    keywords_strong: Vec<String>,
    keywords_basic: Vec<String>,
    keywords_weak: Vec<String>,
    number_glue: Vec<char>,
    numbers: Vec<NumberType>,
    call_syms: Vec<String>,
    symbols: Vec<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct SyntaxFile {
    #[serde(flatten)]
    inner: LiteMap<String, Arc<SyntaxConfig>>,
}

impl SyntaxFile {
    pub fn parse(config_str: &str) -> Result<Self, &'static str> {
        match toml::from_str(config_str) {
            Ok(config) => Ok(config),
            Err(error) => {
                alert!("failed to parse syntax file: {:?}", error.message());
                Err("failed to parse syntax file")
            }
        }
    }

    pub fn get(&self, syntax_name: &str) -> Option<Arc<SyntaxConfig>> {
        self.inner.get(syntax_name).cloned()
    }

    pub fn resolve_ext(&self, extension: &str) -> Option<&str> {
        self
            .inner
            .iter()
            .find(|(_, s)| s.extension == extension)
            .map(|(n, _)| n.as_str())
    }

    pub fn enumerate(&self) -> impl Iterator<Item = &String> {
        self.inner.keys()
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

    pub fn from_str(string: &str) -> Self {
        match string {
            "imixed" => Identifier(Casing::Mixed),
            "ilower" => Identifier(Casing::Lower),
            "iupper" => Identifier(Casing::Upper),
            "kw-strong" => KeywordStrong,
            "kw-basic" => KeywordBasic,
            "kw-weak" => KeywordWeak,
            "spe-str" => StringSpecial,
            "string" => StringNormal,
            "format" => StringFormat,
            "escape" => StringEscape,
            "comment" => Comment,
            "symbol" => Symbol,
            "wspace" => Whitespace,
            "numhex" => Number(NumberType::Hex),
            "numdec" => Number(NumberType::Dec),
            "numbin" => Number(NumberType::Bin),
            "numoct" => Number(NumberType::Oct),
            "cmixed" => Call(Casing::Mixed),
            "clower" => Call(Casing::Lower),
            "cupper" => Call(Casing::Upper),
            other => {
                alert!("invalid token type: {other:?}");
                Comment
            },
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Range {
    pub len: usize,
    pub mode: RangeMode,
}

impl Range {
    pub fn new(len: usize, mode: RangeMode) -> Self {
        Self { len, mode }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LineContext {
    Comment(usize),
    Special(usize),
    String(usize),
}

impl SyntaxConfig {
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

    fn remap(&self, ranges: &mut Vec<Range>) {
        for range in ranges.iter_mut() {
            let in_str = range.mode.as_str();
            if let Some(out_str) = self.remap.get(in_str) {
                range.mode = RangeMode::from_str(out_str);
            }
        }
    }

    pub fn highlight(
        &self,
        start: Option<LineContext>,
        dst: &mut Vec<Range>,
        mut line: &str,
    ) -> Option<LineContext> {
        let line_backup = line;
        dst.clear();

        if let Some(ctx) = start {
            let (str_cfg, mode) = match ctx {
                LineContext::Special(i) => (&self.strings_special[i], StringSpecial),
                LineContext::String(i) => (&self.strings_normal[i], StringNormal),
                LineContext::Comment(i) => (&self.multi_line_comments[i], Comment),
            };

            let Some(offset) = str_cfg.find_end(line) else {
                // this line does not change the context
                dst.push(Range::new(line.len(), mode));
                self.remap(dst);
                return start;
            };

            dst.push(Range::new(offset, mode));
            line = &line[offset..];
        }

        let mut ident_len = 0;

        fn check_push_ident(ident_len: &mut usize, dst: &mut Vec<Range>) {
            let ident_len = take(ident_len);
            if ident_len > 0 {
                dst.push(Range::new(ident_len, Identifier(Casing::Mixed)));
            }
        }

        'reparse: while !line.is_empty() {
            // single line comments

            for prefix in &self.comment_prefix {
                if line.starts_with(prefix) {
                    check_push_ident(&mut ident_len, dst);
                    let mode = Comment;
                    dst.push(Range::new(line.len(), mode));
                    self.remap(dst);
                    return None;
                }
            }


            // strings

            type Builder = fn(usize) -> LineContext;

            let string_specs = [
                (&self.strings_special, StringSpecial, LineContext::Special as Builder),
                (&self.strings_normal, StringNormal, LineContext::String as Builder),
                (&self.multi_line_comments, Comment, LineContext::Comment as Builder),
            ];

            for (cfg_vec, mode, ctx_gen) in string_specs {
                for (i, str_cfg) in cfg_vec.iter().enumerate() {
                    let Some(payload) = line.strip_prefix(&str_cfg.start) else {
                        continue;
                    };

                    check_push_ident(&mut ident_len, dst);

                    let Some(offset) = str_cfg.find_end(payload) else {
                        dst.push(Range::new(line.len(), mode));
                        self.remap(dst);
                        return Some(ctx_gen(i));
                    };

                    let len = str_cfg.start.len() + offset;
                    dst.push(Range::new(len, mode));
                    line = &payload[offset..];
                    continue 'reparse;
                }
            }


            // symbols

            for symbol in &self.symbols {
                if let Some(rest) = line.strip_prefix(symbol) {
                    check_push_ident(&mut ident_len, dst);
                    dst.push(Range::new(symbol.len(), Symbol));
                    line = rest;
                    continue 'reparse;
                }
            }


            // whitespaces

            let mut iter = line.chars();
            let mut c = iter.next().unwrap();

            if c.is_whitespace() {
                let mut len = 0;

                while c.is_whitespace() {
                    len += c.len_utf8();

                    match iter.next() {
                        Some(next) => c = next,
                        None => break,
                    };
                }

                check_push_ident(&mut ident_len, dst);
                dst.push(Range::new(len, Whitespace));
                line = &line[len..];
                continue 'reparse;
            }


            // identifiers

            let charlene = c.len_utf8();
            ident_len += charlene;
            line = &line[charlene..];
        }

        check_push_ident(&mut ident_len, dst);

        // sort identifiers and calls
        line = line_backup;

        for r in 0..dst.len() {
            let range = dst[r];

            if let Identifier(_) = range.mode {
                let slice = &line[..range.len];
                let mut dst_mode;

                if let Some(num_type) = self.classify_number(slice) {
                    dst_mode = Number(num_type);
                } else if let Some(mode) = self.classify_keyword(slice) {
                    dst_mode = mode;
                } else {
                    let casing = Casing::detect(slice);
                    dst_mode = Identifier(casing);

                    if let Some(next) = dst.get(r + 1) {
                        let snippet = &line[range.len..][..next.len];

                        if self.call_syms.iter().any(|s| s == snippet) {
                            dst_mode = Call(casing);
                        }
                    }
                }

                dst[r].mode = dst_mode;
            }

            line = &line[range.len..];
        }

        self.remap(dst);
        None
    }
}

impl StringConfig {
    fn find_end(&self, mut target: &str) -> Option<usize> {
        let mut char_max = 1;
        let mut skipped = 0;

        // detect escaped characters
        while let Some((before, after)) = target.split_once('\\') {
            if before.contains(&self.stop) {
                break;
            }

            let mut skip = before.len() + 1;

            for allowed_esc in &self.escape {
                if after.starts_with(*allowed_esc) {
                    skip += allowed_esc.len_utf8();
                    char_max = 2;
                    break;
                }
            }

            // todo: handle unallowed escape
            skipped += skip;
            target = &target[skip..];
        }

        if let Some(stop_index) = target.find(&self.stop) {
            let len = skipped + stop_index;

            match self.single_char && len > char_max {
                // highlight only the start token as fallback
                true => Some(0),
                false => Some(len + self.stop.len()),
            }
        } else {
            let len = skipped + target.len();

            if self.single_char && len > char_max {
                return Some(0);
            }

            match self.multi_line {
                true => None,
                // fallback: hightlight until end of line
                false => Some(len),
            }
        }
    }
}
