use std::io::{Error, ErrorKind, Result};
use regex::Regex;

pub fn parse_response_ok(lines: Vec<String>) -> Result<()> {
    let ok_regex = Regex::new(r"^([a-zA-Z0-9]+) ([a-zA-Z0-9]+)(.*)").unwrap();
    let last_line = lines.last().unwrap();

    for cap in ok_regex.captures_iter(last_line) {
        let response_type = cap.at(2).unwrap_or("");
        if response_type == "OK" {
            return Ok(());
        }
    }

    return Err(Error::new(ErrorKind::Other, format!("Invalid Response: {}", last_line).to_string()));
}
