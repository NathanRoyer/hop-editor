use super::*;

fn popup(text: String) -> u16 {
    set_dirty();

    let mut stdout = stdout();
    queue!(stdout, Clear(ClearType::All)).unwrap();
    queue!(stdout, MoveTo(8, 4)).unwrap();
    write!(stdout, "{:╌^1$}", "", 40).unwrap();

    let mut y = 5;

    for line in text.split('\n') {
        queue!(stdout, MoveTo(8, y)).unwrap();
        write!(stdout, "{line}").unwrap();
        y += 1;
    }

    let _ = stdout.flush();

    y
}

#[macro_export]
macro_rules! confirm {
    ($entry:expr $(, $arg:expr)* $(,)?) => {
        $crate::interface::popup::_confirm(format!($entry, $($arg),*))
    }
}

pub fn _confirm(mut text: String) -> bool {
    text += "\n\n- Press Enter to validate.\n- Press Escape to cancel.";
    popup(text);

    loop {
        match read().unwrap() {
            Event::Key(e) if !e.is_release() => match e.code {
                KeyCode::Enter => break true,
                KeyCode::Esc => break false,
                _other => (),
            },
            _other => (),
        }
    }
}

#[macro_export]
macro_rules! prompt {
    ($entry:expr $(, $arg:expr)* $(,)?) => {
        $crate::interface::popup::_prompt(format!($entry, $($arg),*))
    }
}

pub fn _prompt(text: String) -> Option<String> {
    let y = popup(text) + 1;

    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut stdout = stdout();

    let validate = loop {
        let rev1 = SetAttribute(Attribute::Reverse);
        let rev2 = SetAttribute(Attribute::NoReverse);
        let (mut c, mut rest) = (' ', "");

        if let Some(first) = suffix.chars().next() {
            c = first;
            rest = &suffix[c.len_utf8()..];
        }

        queue!(stdout, MoveTo(8, y)).unwrap();
        queue!(stdout, Clear(ClearType::UntilNewLine)).unwrap();
        write!(stdout, "> {prefix}").unwrap();
        write!(stdout, "{rev1}{c}{rev2}").unwrap();
        write!(stdout, "{rest}").unwrap();

        let _ = stdout.flush();

        match read().unwrap() {
            Event::Key(e) if !e.is_release() => match e.code {
                KeyCode::Enter => break true,
                KeyCode::Esc => break false,

                KeyCode::Right if !suffix.is_empty() => {
                    prefix.push(suffix.remove(0));
                },
                KeyCode::Left => {
                    prefix.pop().map(|c| suffix.insert(0, c));
                },
                KeyCode::Backspace => _ = prefix.pop(),
                KeyCode::Delete if !suffix.is_empty() => _ = suffix.remove(0),
                KeyCode::Char(c) => prefix.push(c),
                KeyCode::Home => {
                    suffix.insert_str(0, &prefix);
                    prefix.clear();
                },
                KeyCode::End => {
                    prefix += &suffix;
                    suffix.clear();
                },
                _other => (),
            },
            _other => (),
        }
    };

    if validate {
        prefix += &suffix;
        Some(prefix)
    } else {
        None
    }
}
