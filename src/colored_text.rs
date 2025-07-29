use crossterm::style::*;
use std::fmt;

use crate::interface::default_bg_color;
use crate::theme::Theme;

pub type ModeName = &'static str;
pub type PartLen = usize;
pub type Part = (ModeName, PartLen);

#[derive(Copy, Clone)]
pub struct Selection {
    start: usize,
    len: usize,
}

impl Selection {
    pub fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }
}

pub struct ColoredText<'a> {
    selections: &'a [Selection],
    cursors: &'a [usize],
    tab_width_m1: usize,
    parts: &'a [Part],
    theme: &'a Theme,
    max_chars: usize,
    text: &'a str,
}

impl<'a> ColoredText<'a> {
    pub fn new(
        tab_width_m1: usize,
        cursors: &'a [usize],
        parts: &'a [Part],
        selections: &'a [Selection],
        theme: &'a Theme,
        text: &'a str,
    ) -> Self {
        Self {
            parts,
            selections,
            theme,
            text,
            cursors,
            tab_width_m1,
            max_chars: 0,
        }
    }

    pub fn set_max(&mut self, max: usize) {
        self.max_chars = max;
    }
}

fn write_cursor(f: &mut fmt::Formatter, c: char) -> Result<(), fmt::Error> {
    let rev1 = SetAttribute(Attribute::Reverse);
    let rev2 = SetAttribute(Attribute::NoReverse);
    write!(f, "{rev1}{c}{rev2}")
}

impl<'a> fmt::Display for ColoredText<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let selected = Color::from((80, 80, 80));
        let mut iter_cursor = self.cursors.iter();
        let mut iter_sel = self.selections.iter();
        let mut cursor = iter_cursor.next();
        let mut next_sel = iter_sel.next();
        let mut processed_chars = 0;
        let mut printed_chars = 0;
        let mut overflow = false;
        let mut byte_offset = 0;
        let mut sel_end = None;

        for (mode_name, byte_len) in self.parts {
            let end = byte_offset + byte_len;
            let text = &self.text[byte_offset..end];
            byte_offset = end;

            let color = self.theme.get_ansi(mode_name);
            write!(f, "{}", SetForegroundColor(color))?;

            for mut new_char in text.chars() {
                let was_tab = new_char == '\t';
                let mut added_chars = 1;

                if was_tab {
                    added_chars += self.tab_width_m1;
                    new_char = ' ';
                }

                overflow = printed_chars + added_chars >= self.max_chars;

                if overflow {
                    break;
                }

                if sel_end.is_some_and(|i| i <= processed_chars) {
                    write!(f, "{}", SetBackgroundColor(default_bg_color()))?;
                    next_sel = iter_sel.next();
                    sel_end.take();
                }

                if let Some(sel) = next_sel {
                    if sel.start <= processed_chars && sel_end.is_none() {
                        write!(f, "{}", SetBackgroundColor(selected))?;
                        sel_end = Some(sel.start + sel.len);
                    }
                }

                if Some(processed_chars) == cursor.copied() {
                    cursor = iter_cursor.next();
                    write_cursor(f, new_char)?;
                } else {
                    write!(f, "{new_char}")?;
                }

                if was_tab {
                    let _ = write!(f, "{:^1$}", "", self.tab_width_m1);
                }

                printed_chars += added_chars;
                processed_chars += 1;
            }

            if overflow {
                write!(f, "{}…", SetForegroundColor(Color::Reset))?;
                break;
            }
        }

        write!(f, "{}", SetBackgroundColor(default_bg_color()))?;
        if self.cursors.contains(&byte_offset) & !overflow {
            write_cursor(f, ' ')?;
        }

        Ok(())
    }
}
