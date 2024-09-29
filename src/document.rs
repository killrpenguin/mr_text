#![allow(unused_imports, unused_variables)]
#![allow(dead_code)]

use ropey::Rope;
use std::{io::BufWriter, ops::RangeBounds};

pub trait Doc {
    fn parse_file_name(name: &str) -> (String, String) {
        if let Some(pos) = name.chars().position(|letter| letter == '.') {
            (name[..pos].to_string(), name[pos..].to_string())
        } else {
            (name.to_string(), ".txt".to_string())
        }
    }
}

pub struct NewDocument {
    rope: Rope,
    file_name: String,
    file_ext: String,
    char_count: usize,
    line_count: usize,
}

impl Doc for NewDocument {}

impl NewDocument {
    pub fn new() -> Self {
        NewDocument {
            rope: Rope::new(),
            file_name: "new.txt".to_string(),
            file_ext: ".txt".to_string(),
            char_count: 0,
            line_count: 0,
        }
    }

    pub fn save_file(self, name: Option<&str>) -> std::io::Result<Document> {
        let fl_nm = if let Some(name) = name {
            Self::parse_file_name(name)
        } else {
            (self.file_name, self.file_ext)
        };
        let file_name = format!("{}{}", fl_nm.0, fl_nm.1);
        let file = match std::fs::File::create_new(&file_name) {
            Ok(file) => file,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        let buf_writer = BufWriter::new(file);
        match self.rope.write_to(buf_writer) {
            Ok(()) => {}
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        Ok(Document {
            rope: self.rope,
            file_name,
            file_ext: fl_nm.1,
            char_count: self.char_count,
            line_count: self.line_count,
        })
    }

    pub fn update_doc_info(&mut self) {
        self.char_count = self.rope.len_chars();
        self.line_count = self.rope.len_lines()
    }

    pub fn insert(&mut self, text: &str) {
        self.rope.insert(self.rope.len_chars(), text)
    }

    pub fn remove<R>(&mut self, char_range: R)
    where
        R: RangeBounds<usize>,
    {
        self.rope.remove(char_range)
    }
}

pub struct Document {
    file_name: std::string::String,
    file_ext: std::string::String,
    char_count: usize,
    line_count: usize,
    rope: Rope,
}

impl Doc for Document {}

impl Document {
    //TODO: Change File to Open builder.
    pub fn load_file(name: &str) -> std::io::Result<Self> {
        let fl_nm = if let Some(pos) = name.chars().position(|letter| letter == '.') {
            (name[..pos].to_string(), name[pos..].to_string())
        } else {
            (name.to_string(), ".txt".to_string())
        };
        let file_name = format!("{}{}", fl_nm.0, fl_nm.1);
        let file = match std::fs::File::open(&file_name) {
            Ok(file) => file,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        let reader = std::io::BufReader::new(file);
        let rope = match Rope::from_reader(reader) {
            Ok(rope) => rope,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        Ok(Document {
            file_name,
            file_ext: fl_nm.1,
            char_count: rope.len_chars(),
            line_count: rope.len_lines(),
            rope,
        })
    }

    //TODO: Change File to Open builder.
    pub fn save_file(&self)  -> std::io::Result<()> {
        let file = match std::fs::File::create_new(&self.file_name) {
            Ok(file) => file,
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        let buf_writer = BufWriter::new(file);
        match self.rope.write_to(buf_writer) {
            Ok(()) => {}
            Err(_) => return Err(std::io::Error::last_os_error()),
        };
        Ok(())
    }


    pub fn update_doc_info(&mut self) {
        self.char_count = self.rope.len_chars();
        self.line_count = self.rope.len_lines()
    }

    pub fn insert(&mut self, text: &str) {
        self.rope.insert(self.rope.len_chars(), text)
    }

    pub fn remove<R>(&mut self, char_range: R)
    where
        R: RangeBounds<usize>,
    {
        self.rope.remove(char_range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_file() {
        let doc = Document::load_file("./text.txt").unwrap();
        assert_eq!(20, doc.line_count);
    }
}
