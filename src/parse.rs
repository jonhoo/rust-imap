use std::io::{Error, ErrorKind, Result};
use regex::Regex;

pub fn parse_capability(lines: Vec<String>) -> Result<Vec<String>> {
    let capability_regex = Regex::new(r"^\* CAPABILITY (.*)\r\n").unwrap();

    //Check Ok
    match parse_response_ok(lines.clone()) {
        Ok(_) => (),
        Err(e) => return Err(e)
    };

    for line in lines.iter() {
        if capability_regex.is_match(line) {
            let cap = capability_regex.captures(line).unwrap();
            let capabilities_str = cap.at(1).unwrap();
            return Ok(capabilities_str.split(' ').map(|x| x.to_string()).collect());
        }
    }

    Err(Error::new(ErrorKind::Other, "Error parsing capabilities response"))
}

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
