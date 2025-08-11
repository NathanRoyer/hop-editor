use std::process::Command;
use crate::config::internal_clipboard;
use super::*;

const TMP_PATH: &str = "/tmp/hop-clipboard.txt";
const DELIMITER: &str = " \n";

impl Tab {
    pub fn copy(&mut self) {
        let cursors = self.cursors.len();
        let mut text = String::new();

        for c in 0..cursors {
            self.extract_selection(c, &mut text);

            if c + 1 < cursors {
                text += DELIMITER;
            }
        }

        if internal_clipboard() {
            self.internal_clipboard = text;
            return;
        }

        if let Err(error) = fs::write(TMP_PATH, text) {
            alert!("failed to write clipboard-file ({TMP_PATH}):\n{error:?}");
            return;
        }

        try_exec(true);
    }

    pub fn cut(&mut self) {
        self.copy();
        self.erase_selection();
    }

    pub fn paste(&mut self) {
        let text = if internal_clipboard() {
            self.internal_clipboard.clone()
        } else {
            try_exec(false);

            let Ok(contents) = fs::read_to_string(TMP_PATH) else {
                alert!("failed to read clipboard-file ({TMP_PATH})");
                return;
            };

            contents
        };

        let cursors = self.cursors.len();

        if cursors > 1 {
            let regions = text.split(DELIMITER).count();

            if regions != cursors {
                alert!("cannot paste: {regions} clipboard regions but {cursors} cursors");
                return;
            }

            self.prepare_insertion();
            self.erase_selection();

            let iter = text.split(DELIMITER);
            for (c, region) in iter.enumerate() {
                self.insert_text_cursor(c, region);
            }

            self.modified = true;
        } else {
            self.insert_text(&text);
        }
    }
}

fn try_exec(copy: bool) {
    let mut success = false;

    let maybe_file = match copy {
        true => fs::File::open(TMP_PATH),
        false => fs::File::create(TMP_PATH),
    };

    let Ok(file) = maybe_file else {
        alert!("failed to open clipboard-file ({TMP_PATH})");
        return;
    };

    let candidates: [(&str, &[&str]); 3] = match copy {
        true => [("wl-copy", &[]), ("xclip", &[]), ("pbcopy", &[])],
        false => [("wl-paste", &["-n"]), ("xclip", &["-o"]), ("pbpaste", &[])],
    };

    for (command, args) in candidates {
        let buffer = file.try_clone().unwrap();
        let mut cmd = Command::new(command);

        match copy {
            true => cmd.stdin(buffer).args(args),
            false => cmd.stdout(buffer).args(args),
        };

        let Ok(mut child) = cmd.spawn() else {
            continue;
        };

        let Ok(exit) = child.wait() else {
            continue;
        };

        if exit.success() {
            success = true;
            break;
        }
    }

    if !success {
        let ln1 = "failed to use wl-clipboard, xclip or macOS equivalents.";
        let ln2 = "please make sure at least one of these works.";
        let ln3 = "alternatively, set `internal-clipboard` to `true` in config.";
        alert!("{ln1}\n{ln2}\n{ln3}");
    }
}
