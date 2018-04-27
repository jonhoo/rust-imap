use imap_proto::{self, Response};
use nom::IResult;
use regex::Regex;

use super::error::{Error, ParseError, Result};
use super::types::*;

pub fn parse_authenticate_response(line: String) -> Result<String> {
    let authenticate_regex = Regex::new("^+(.*)\r\n").unwrap();

    for cap in authenticate_regex.captures_iter(line.as_str()) {
        let data = cap.get(1).map(|x| x.as_str()).unwrap_or("");
        return Ok(String::from(data));
    }

    Err(Error::Parse(ParseError::Authentication(line)))
}

enum MapOrNot<T> {
    Map(T),
    Not(Response<'static>),
}

unsafe fn parse_many<T, F>(lines: Vec<u8>, mut map: F) -> ZeroCopyResult<Vec<T>>
where
    F: FnMut(Response<'static>) -> MapOrNot<T>,
{
    let f = |mut lines| {
        let mut things = Vec::new();
        loop {
            match imap_proto::parse_response(lines) {
                IResult::Done(rest, resp) => {
                    lines = rest;

                    match map(resp) {
                        MapOrNot::Map(t) => things.push(t),
                        MapOrNot::Not(resp) => break Err(resp.into()),
                    }

                    if lines.is_empty() {
                        break Ok(things);
                    }
                }
                _ => {
                    break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }
    };

    ZeroCopy::new(lines, f)
}

pub fn parse_names(lines: Vec<u8>) -> ZeroCopyResult<Vec<Name>> {
    use imap_proto::MailboxDatum;
    let f = |resp| match resp {
        // https://github.com/djc/imap-proto/issues/4
        Response::MailboxData(MailboxDatum::List {
            flags,
            delimiter,
            name,
        })
        | Response::MailboxData(MailboxDatum::SubList {
            flags,
            delimiter,
            name,
        }) => MapOrNot::Map(Name {
            attributes: flags,
            delimiter,
            name,
        }),
        resp => MapOrNot::Not(resp),
    };

    unsafe { parse_many(lines, f) }
}

pub fn parse_fetches(lines: Vec<u8>) -> ZeroCopyResult<Vec<Fetch>> {
    let f = |resp| match resp {
        Response::Fetch(num, attrs) => {
            let mut fetch = Fetch {
                message: num,
                flags: vec![],
                uid: None,
                rfc822_header: None,
                rfc822: None,
            };

            for attr in attrs {
                use imap_proto::AttributeValue;
                match attr {
                    AttributeValue::Flags(flags) => {
                        fetch.flags.extend(flags);
                    }
                    AttributeValue::Uid(uid) => fetch.uid = Some(uid),
                    AttributeValue::Rfc822(rfc) => fetch.rfc822 = rfc,
                    AttributeValue::Rfc822Header(rfc) => fetch.rfc822_header = rfc,
                    _ => {}
                }
            }

            MapOrNot::Map(fetch)
        }
        resp => MapOrNot::Not(resp),
    };

    unsafe { parse_many(lines, f) }
}

pub fn parse_capabilities(lines: Vec<u8>) -> ZeroCopyResult<Capabilities> {
    let f = |mut lines| {
        use std::collections::HashSet;
        let mut caps = HashSet::new();
        loop {
            match imap_proto::parse_response(lines) {
                IResult::Done(rest, Response::Capabilities(c)) => {
                    lines = rest;
                    caps.extend(c);

                    if lines.is_empty() {
                        break Ok(Capabilities(caps));
                    }
                }
                IResult::Done(_, resp) => {
                    break Err(resp.into());
                }
                _ => {
                    break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }
    };

    unsafe { ZeroCopy::new(lines, f) }
}

pub fn parse_mailbox(mut lines: &[u8]) -> Result<Mailbox> {
    let mut mailbox = Mailbox::default();

    loop {
        match imap_proto::parse_response(lines) {
            IResult::Done(rest, Response::Data { status, code, .. }) => {
                lines = rest;

                if let imap_proto::Status::Ok = status {
                } else {
                    // how can this happen for a Response::Data?
                    unreachable!();
                }

                use imap_proto::ResponseCode;
                match code {
                    Some(ResponseCode::UidValidity(uid)) => {
                        mailbox.uid_validity = Some(uid);
                    }
                    Some(ResponseCode::UidNext(unext)) => {
                        mailbox.uid_next = Some(unext);
                    }
                    Some(ResponseCode::Unseen(n)) => {
                        mailbox.unseen = Some(n);
                    }
                    Some(ResponseCode::PermanentFlags(flags)) => {
                        mailbox
                            .permanent_flags
                            .extend(flags.into_iter().map(|s| s.to_string()));
                    }
                    _ => {}
                }
            }
            IResult::Done(rest, Response::MailboxData(m)) => {
                lines = rest;

                use imap_proto::MailboxDatum;
                match m {
                    MailboxDatum::Status { .. } => {
                        // TODO: we probably want to expose statuses too
                    }
                    MailboxDatum::Exists(e) => {
                        mailbox.exists = e;
                    }
                    MailboxDatum::Recent(r) => {
                        mailbox.recent = r;
                    }
                    MailboxDatum::Flags(flags) => {
                        mailbox
                            .flags
                            .extend(flags.into_iter().map(|s| s.to_string()));
                    }
                    MailboxDatum::SubList { .. } | MailboxDatum::List { .. } => {}
                }
            }
            IResult::Done(_, resp) => {
                break Err(resp.into());
            }
            _ => {
                break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
            }
        }

        if lines.is_empty() {
            break Ok(mailbox);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capability_test() {
        let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
        let lines = b"* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n";
        let capabilities = parse_capabilities(lines.to_vec()).unwrap();
        assert_eq!(capabilities.len(), 4);
        for e in expected_capabilities {
            assert!(capabilities.has(e));
        }
    }

    #[test]
    #[should_panic]
    fn parse_capability_invalid_test() {
        let lines = b"* JUNK IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n";
        parse_capabilities(lines.to_vec()).unwrap();
    }

    #[test]
    fn parse_names_test() {
        let lines = b"* LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n";
        let names = parse_names(lines.to_vec()).unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].attributes(), &["\\HasNoChildren"]);
        assert_eq!(names[0].delimiter(), ".");
        assert_eq!(names[0].name(), "INBOX");
    }

    #[test]
    fn parse_fetches_test() {
        let lines = b"\
                    * 24 FETCH (FLAGS (\\Seen) UID 4827943)\r\n\
                    * 25 FETCH (FLAGS (\\Seen))\r\n";
        let fetches = parse_fetches(lines.to_vec()).unwrap();
        assert_eq!(fetches.len(), 2);
        assert_eq!(fetches[0].message, 24);
        assert_eq!(fetches[0].flags(), &["\\Seen"]);
        assert_eq!(fetches[0].uid, Some(4827943));
        assert_eq!(fetches[0].rfc822(), None);
        assert_eq!(fetches[1].message, 25);
        assert_eq!(fetches[1].flags(), &["\\Seen"]);
        assert_eq!(fetches[1].uid, None);
        assert_eq!(fetches[1].rfc822(), None);
    }
}
