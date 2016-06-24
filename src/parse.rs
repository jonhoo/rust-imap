use std::io::{self};
use regex::Regex;

use super::mailbox::Mailbox;
use super::error::{Error, Result};

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

    Err(Error::Io(io::Error::new(io::ErrorKind::Other, "Error parsing capabilities response")))
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

    Err(Error::Io(io::Error::new(io::ErrorKind::Other, format!("Invalid Response: {}", last_line).to_string())))
}

pub fn parse_select_or_examine(lines: Vec<String>) -> Result<Mailbox> {
    let exists_regex = Regex::new(r"^\* (\d+) EXISTS\r\n").unwrap();

    let recent_regex = Regex::new(r"^\* (\d+) RECENT\r\n").unwrap();

    let flags_regex = Regex::new(r"^\* FLAGS (.+)\r\n").unwrap();

    let unseen_regex = Regex::new(r"^\* OK \[UNSEEN (\d+)\](.*)\r\n").unwrap();

    let uid_validity_regex = Regex::new(r"^\* OK \[UIDVALIDITY (\d+)\](.*)\r\n").unwrap();

    let uid_next_regex = Regex::new(r"^\* OK \[UIDNEXT (\d+)\](.*)\r\n").unwrap();

    let permanent_flags_regex = Regex::new(r"^\* OK \[PERMANENTFLAGS (.+)\](.*)\r\n").unwrap();

    //Check Ok
    match parse_response_ok(lines.clone()) {
        Ok(_) => (),
        Err(e) => return Err(e)
    };

    let mut mailbox = Mailbox::default();

    for line in lines.iter() {
        if exists_regex.is_match(line) {
            let cap = exists_regex.captures(line).unwrap();
            mailbox.exists = cap.at(1).unwrap().parse::<u32>().unwrap();
        } else if recent_regex.is_match(line) {
            let cap = recent_regex.captures(line).unwrap();
            mailbox.recent = cap.at(1).unwrap().parse::<u32>().unwrap();
        } else if flags_regex.is_match(line) {
            let cap = flags_regex.captures(line).unwrap();
            mailbox.flags = cap.at(1).unwrap().to_string();
        } else if unseen_regex.is_match(line) {
            let cap = unseen_regex.captures(line).unwrap();
            mailbox.unseen = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
        } else if uid_validity_regex.is_match(line) {
            let cap = uid_validity_regex.captures(line).unwrap();
            mailbox.uid_validity = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
        } else if uid_next_regex.is_match(line) {
            let cap = uid_next_regex.captures(line).unwrap();
            mailbox.uid_next = Some(cap.at(1).unwrap().parse::<u32>().unwrap());
        } else if permanent_flags_regex.is_match(line) {
            let cap = permanent_flags_regex.captures(line).unwrap();
            mailbox.permanent_flags = Some(cap.at(1).unwrap().to_string());
        }
    }

    Ok(mailbox)
}
