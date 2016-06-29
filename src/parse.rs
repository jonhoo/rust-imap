use regex::Regex;

use super::mailbox::Mailbox;
use super::error::{Error, ParseError, Result};

pub fn parse_authenticate_response(line: String) -> Result<String> {
    let authenticate_regex = Regex::new("^+(.*)\r\n").unwrap();

    for cap in authenticate_regex.captures_iter(line.as_str()) {
        let data = cap.at(1).unwrap_or("");
        return Ok(String::from(data));
    }

    Err(Error::Parse(ParseError::Authentication(line)))
}

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

    Err(Error::Parse(ParseError::Capability(lines)))
}

pub fn parse_response_ok(lines: Vec<String>) -> Result<()> {
    match parse_response(lines) {
        Ok(_) => Ok(()),
        Err(e) => return Err(e)
    }
}

pub fn parse_response(lines: Vec<String>) -> Result<Vec<String>> {
    let regex = Regex::new(r"^([a-zA-Z0-9]+) (OK|NO|BAD)(.*)").unwrap();
    let last_line = match lines.last() {
        Some(l) => l,
        None => return Err(Error::Parse(ParseError::StatusResponse(lines.clone())))
    };

    for cap in regex.captures_iter(last_line) {
        let response_type = cap.at(2).unwrap_or("");
        match response_type {
            "OK" => return Ok(lines.clone()),
            "BAD" => return Err(Error::BadResponse(lines.clone())),
            "NO" => return Err(Error::NoResponse(lines.clone())),
            _ => {}
        }
    }

    Err(Error::Parse(ParseError::StatusResponse(lines.clone())))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capability_test() {
        let expected_capabilities = vec![String::from("IMAP4rev1"), String::from("STARTTLS"), String::from("AUTH=GSSAPI"), String::from("LOGINDISABLED")];
        let lines = vec![String::from("* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n"), String::from("a1 OK CAPABILITY completed\r\n")];
        let capabilities = parse_capability(lines).unwrap();
        assert!(capabilities == expected_capabilities, "Unexpected capabilities parse response");
    }

    #[test]
    #[should_panic]
    fn parse_capability_invalid_test() {
        let lines = vec![String::from("* JUNK IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n"), String::from("a1 OK CAPABILITY completed\r\n")];
        parse_capability(lines).unwrap();
    }

    #[test]
    fn parse_response_test() {
        let lines = vec![String::from("* LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n"), String::from("a2 OK List completed.\r\n")];
        let expected_lines = lines.clone();
        let actual_lines = parse_response(lines).unwrap();
        assert!(expected_lines == actual_lines, "Unexpected parse response");
    }

    #[test]
    #[should_panic]
    fn parse_response_invalid_test() {
        let lines = vec![String::from("* LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n"), String::from("a2 BAD broken.\r\n")];
        parse_response(lines).unwrap();
    }

    #[test]
    #[should_panic]
    fn parse_response_invalid2_test() {
        let lines = vec![String::from("* LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n"), String::from("a2 broken.\r\n")];
        parse_response(lines).unwrap();
    }
}
