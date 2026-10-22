use crate::{TMP_CLIPBOARD_PATH, try_clipboard_command};
use crate::config::internal_clipboard;
use super::*;

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

        if let Err(error) = fs::write(TMP_CLIPBOARD_PATH, text) {
            alert!("failed to write clipboard-file ({TMP_CLIPBOARD_PATH}):\n{error:?}");
            return;
        }

        try_clipboard_command(true);
    }

    pub fn cut(&mut self) {
        self.copy();
        self.erase_selection();
    }

    pub fn paste(&mut self) {
        let text = if internal_clipboard() {
            self.internal_clipboard.clone()
        } else {
            try_clipboard_command(false);

            let Ok(contents) = fs::read_to_string(TMP_CLIPBOARD_PATH) else {
                alert!("failed to read clipboard-file ({TMP_CLIPBOARD_PATH})");
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
