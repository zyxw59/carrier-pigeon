pub struct TextBox {
    /// Text before the virtual cursor
    pre_cursor: String,
    /// Text after the virtual cursor
    post_cursor: String,
    /// Offset of the real cursor relative to the virtual cursor. The virtual cursor is updated to
    /// match the real cursor on insert or delete operations.
    cursor_offset: isize,
}

impl TextBox {
    pub fn insert_before_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.update_cursor();
        self.pre_cursor += text;
    }

    pub fn move_cursor(&mut self, offset: isize) {
        self.cursor_offset += offset;
    }

    fn update_cursor(&mut self) {
        match self.cursor_offset {
            // already updated
            0 => {}
            // move the virtual cursor backwards
            bytes @ ..=-1 => {
                self.pre_cursor += &self.post_cursor[..bytes.unsigned_abs()];
                self.post_cursor = self.post_cursor.split_off(bytes.unsigned_abs());
            }
            // move the virtual cursor forwards
            bytes @ 1.. => {
                self.post_cursor = self
                    .pre_cursor
                    .split_off(self.pre_cursor.len().saturating_add_signed(-bytes))
                    + &self.post_cursor;
            }
        }
        self.cursor_offset = 0;
    }
}
