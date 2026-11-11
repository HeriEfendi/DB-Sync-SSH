#[derive(Default)]
pub struct MysqlDumpRowCounter {
    word: Vec<u8>,
    in_insert: bool,
    in_values: bool,
    quote: Option<u8>,
    escaped: bool,
    tuple_depth: usize,
    expect_tuple: bool,
    pub total_rows: usize,
}

impl MysqlDumpRowCounter {
    pub fn push(&mut self, chunk: &[u8]) -> usize {
        let previous_total = self.total_rows;
        for &byte in chunk {
            if let Some(quote) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if byte == b'\\' {
                    self.escaped = true;
                } else if byte == quote {
                    self.quote = None;
                }
                continue;
            }

            if matches!(byte, b'\'' | b'"' | b'`') {
                self.finish_word();
                self.quote = Some(byte);
                continue;
            }

            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.word.push(byte.to_ascii_uppercase());
                continue;
            }

            self.finish_word();
            match byte {
                b'(' if self.in_values => {
                    if self.tuple_depth == 0 && self.expect_tuple {
                        self.total_rows += 1;
                        self.expect_tuple = false;
                    }
                    self.tuple_depth += 1;
                }
                b')' if self.in_values && self.tuple_depth > 0 => {
                    self.tuple_depth -= 1;
                }
                b',' if self.in_values && self.tuple_depth == 0 => {
                    self.expect_tuple = true;
                }
                b';' => {
                    self.in_insert = false;
                    self.in_values = false;
                    self.expect_tuple = false;
                    self.tuple_depth = 0;
                }
                _ => {}
            }
        }
        self.total_rows - previous_total
    }

    fn finish_word(&mut self) {
        if self.word.is_empty() {
            return;
        }

        match self.word.as_slice() {
            b"INSERT" | b"REPLACE" => {
                self.in_insert = true;
                self.in_values = false;
                self.expect_tuple = false;
                self.tuple_depth = 0;
            }
            b"VALUES" if self.in_insert => {
                self.in_values = true;
                self.expect_tuple = true;
                self.tuple_depth = 0;
            }
            _ => {}
        }
        self.word.clear();
    }
}
