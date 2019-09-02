use imap_proto::{self, MailboxDatum, Response};
use regex::Regex;
use std::collections::HashSet;
use std::sync::mpsc;

use super::error::{Error, ParseError, Result};
use super::types::*;

pub fn parse_authenticate_response(line: String) -> Result<String> {
    let authenticate_regex = Regex::new("^\\+ (.*)\r\n").unwrap();

    if let Some(cap) = authenticate_regex.captures_iter(line.as_str()).next() {
        let data = cap.get(1).map(|x| x.as_str()).unwrap_or("");
        return Ok(String::from(data));
    }

    Err(Error::Parse(ParseError::Authentication(line, None)))
}

enum MapOrNot<T> {
    Map(T),
    Not(Response<'static>),
    #[allow(dead_code)]
    Ignore,
}

unsafe fn parse_many<T, F>(
    lines: Vec<u8>,
    mut map: F,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> ZeroCopyResult<Vec<T>>
where
    F: FnMut(Response<'static>) -> Result<MapOrNot<T>>,
{
    let f = |mut lines: &'static [u8]| {
        let mut things = Vec::new();
        loop {
            if lines.is_empty() {
                break Ok(things);
            }

            match imap_proto::parse_response(lines) {
                Ok((rest, resp)) => {
                    lines = rest;

                    match map(resp)? {
                        MapOrNot::Map(t) => things.push(t),
                        MapOrNot::Not(resp) => match handle_unilateral(resp, unsolicited) {
                            Some(Response::Fetch(..)) => continue,
                            Some(resp) => break Err(resp.into()),
                            None => {}
                        },
                        MapOrNot::Ignore => continue,
                    }
                }
                _ => {
                    break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }
        }
    };

    ZeroCopy::make(lines, f)
}

pub fn parse_names(
    lines: Vec<u8>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> ZeroCopyResult<Vec<Name>> {
    let f = |resp| match resp {
        // https://github.com/djc/imap-proto/issues/4
        Response::MailboxData(MailboxDatum::List {
            flags,
            delimiter,
            name,
        }) => Ok(MapOrNot::Map(Name {
            attributes: flags.into_iter().map(NameAttribute::from).collect(),
            delimiter,
            name,
        })),
        resp => Ok(MapOrNot::Not(resp)),
    };

    unsafe { parse_many(lines, f, unsolicited) }
}

pub fn parse_fetches(
    lines: Vec<u8>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> ZeroCopyResult<Vec<Fetch>> {
    let f = |resp| match resp {
        Response::Fetch(num, attrs) => {
            let mut fetch = Fetch {
                message: num,
                flags: vec![],
                uid: None,
                size: None,
                fetch: attrs,
            };

            // set some common fields eaglery
            for attr in &fetch.fetch {
                use imap_proto::AttributeValue;
                match attr {
                    AttributeValue::Flags(flags) => {
                        fetch.flags.extend(flags.iter().cloned().map(Flag::from));
                    }
                    AttributeValue::Uid(uid) => fetch.uid = Some(*uid),
                    AttributeValue::Rfc822Size(sz) => fetch.size = Some(*sz),
                    _ => {}
                }
            }

            Ok(MapOrNot::Map(fetch))
        }
        resp => Ok(MapOrNot::Not(resp)),
    };

    unsafe { parse_many(lines, f, unsolicited) }
}

pub fn parse_expunge(
    lines: Vec<u8>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> Result<Vec<u32>> {
    let f = |resp| match resp {
        Response::Expunge(id) => Ok(MapOrNot::Map(id)),
        resp => Ok(MapOrNot::Not(resp)),
    };

    unsafe { parse_many(lines, f, unsolicited).map(|ids| ids.take()) }
}

pub fn parse_capabilities(
    lines: Vec<u8>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> ZeroCopyResult<Capabilities> {
    let f = |mut lines| {
        let mut caps = HashSet::new();
        loop {
            match imap_proto::parse_response(lines) {
                Ok((rest, Response::Capabilities(c))) => {
                    lines = rest;
                    caps.extend(c);
                }
                Ok((rest, data)) => {
                    lines = rest;
                    if let Some(resp) = handle_unilateral(data, unsolicited) {
                        break Err(resp.into());
                    }
                }
                _ => {
                    break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                }
            }

            if lines.is_empty() {
                break Ok(Capabilities(caps));
            }
        }
    };

    unsafe { ZeroCopy::make(lines, f) }
}

pub fn parse_noop(
    lines: Vec<u8>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> Result<()> {
    let mut lines: &[u8] = &lines;

    loop {
        if lines.is_empty() {
            break Ok(());
        }

        match imap_proto::parse_response(lines) {
            Ok((rest, data)) => {
                lines = rest;
                if let Some(resp) = handle_unilateral(data, unsolicited) {
                    break Err(resp.into());
                }
            }
            _ => {
                break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
            }
        }
    }
}

pub fn parse_mailbox(
    mut lines: &[u8],
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> Result<Mailbox> {
    let mut mailbox = Mailbox::default();

    loop {
        match imap_proto::parse_response(lines) {
            Ok((rest, Response::Data { status, code, .. })) => {
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
                            .extend(flags.into_iter().map(String::from).map(Flag::from));
                    }
                    _ => {}
                }
            }
            Ok((rest, Response::MailboxData(m))) => {
                lines = rest;

                match m {
                    MailboxDatum::Status { mailbox, status } => {
                        unsolicited
                            .send(UnsolicitedResponse::Status {
                                mailbox: mailbox.into(),
                                attributes: status,
                            })
                            .unwrap();
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
                            .extend(flags.into_iter().map(String::from).map(Flag::from));
                    }
                    MailboxDatum::List { .. } => {}
                }
            }
            Ok((rest, Response::Expunge(n))) => {
                lines = rest;
                unsolicited.send(UnsolicitedResponse::Expunge(n)).unwrap();
            }
            Ok((_, resp)) => {
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

pub fn parse_ids(
    lines: &[u8],
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> Result<HashSet<u32>> {
    let mut lines = &lines[..];
    let mut ids = HashSet::new();
    loop {
        if lines.is_empty() {
            break Ok(ids);
        }

        match imap_proto::parse_response(lines) {
            Ok((rest, Response::IDs(c))) => {
                lines = rest;
                ids.extend(c);
            }
            Ok((rest, data)) => {
                lines = rest;
                if let Some(resp) = handle_unilateral(data, unsolicited) {
                    break Err(resp.into());
                }
            }
            _ => {
                break Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
            }
        }
    }
}

// check if this is simply a unilateral server response
// (see Section 7 of RFC 3501):
fn handle_unilateral<'a>(
    res: Response<'a>,
    unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
) -> Option<Response<'a>> {
    match res {
        Response::MailboxData(MailboxDatum::Status { mailbox, status }) => {
            unsolicited
                .send(UnsolicitedResponse::Status {
                    mailbox: mailbox.into(),
                    attributes: status,
                })
                .unwrap();
        }
        Response::MailboxData(MailboxDatum::Recent(n)) => {
            unsolicited.send(UnsolicitedResponse::Recent(n)).unwrap();
        }
        Response::MailboxData(MailboxDatum::Exists(n)) => {
            unsolicited.send(UnsolicitedResponse::Exists(n)).unwrap();
        }
        Response::Expunge(n) => {
            unsolicited.send(UnsolicitedResponse::Expunge(n)).unwrap();
        }
        res => {
            return Some(res);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capability_test() {
        let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
        let lines = b"* CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n";
        let (mut send, recv) = mpsc::channel();
        let capabilities = parse_capabilities(lines.to_vec(), &mut send).unwrap();
        // shouldn't be any unexpected responses parsed
        assert!(recv.try_recv().is_err());
        assert_eq!(capabilities.len(), 4);
        for e in expected_capabilities {
            assert!(capabilities.has_str(e));
        }
    }

    #[test]
    #[should_panic]
    fn parse_capability_invalid_test() {
        let (mut send, recv) = mpsc::channel();
        let lines = b"* JUNK IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n";
        parse_capabilities(lines.to_vec(), &mut send).unwrap();
        assert!(recv.try_recv().is_err());
    }

    #[test]
    fn parse_names_test() {
        let lines = b"* LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n";
        let (mut send, recv) = mpsc::channel();
        let names = parse_names(lines.to_vec(), &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        assert_eq!(names.len(), 1);
        assert_eq!(
            names[0].attributes(),
            &[NameAttribute::from("\\HasNoChildren")]
        );
        assert_eq!(names[0].delimiter(), Some("."));
        assert_eq!(names[0].name(), "INBOX");
    }

    #[test]
    fn parse_fetches_empty() {
        let lines = b"";
        let (mut send, recv) = mpsc::channel();
        let fetches = parse_fetches(lines.to_vec(), &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        assert!(fetches.is_empty());
    }

    #[test]
    fn parse_fetches_test() {
        let lines = b"\
                    * 24 FETCH (FLAGS (\\Seen) UID 4827943)\r\n\
                    * 25 FETCH (FLAGS (\\Seen))\r\n";
        let (mut send, recv) = mpsc::channel();
        let fetches = parse_fetches(lines.to_vec(), &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        assert_eq!(fetches.len(), 2);
        assert_eq!(fetches[0].message, 24);
        assert_eq!(fetches[0].flags(), &[Flag::Seen]);
        assert_eq!(fetches[0].uid, Some(4827943));
        assert_eq!(fetches[0].body(), None);
        assert_eq!(fetches[0].header(), None);
        assert_eq!(fetches[1].message, 25);
        assert_eq!(fetches[1].flags(), &[Flag::Seen]);
        assert_eq!(fetches[1].uid, None);
        assert_eq!(fetches[1].body(), None);
        assert_eq!(fetches[1].header(), None);
    }

    #[test]
    fn parse_fetches_w_unilateral() {
        // https://github.com/mattnenterprise/rust-imap/issues/81
        let lines = b"\
            * 37 FETCH (UID 74)\r\n\
            * 1 RECENT\r\n";
        let (mut send, recv) = mpsc::channel();
        let fetches = parse_fetches(lines.to_vec(), &mut send).unwrap();
        assert_eq!(recv.try_recv(), Ok(UnsolicitedResponse::Recent(1)));
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].message, 37);
        assert_eq!(fetches[0].uid, Some(74));
    }

    #[test]
    fn parse_names_w_unilateral() {
        let lines = b"\
                    * LIST (\\HasNoChildren) \".\" \"INBOX\"\r\n\
                    * 4 EXPUNGE\r\n";
        let (mut send, recv) = mpsc::channel();
        let names = parse_names(lines.to_vec(), &mut send).unwrap();

        assert_eq!(recv.try_recv().unwrap(), UnsolicitedResponse::Expunge(4));

        assert_eq!(names.len(), 1);
        assert_eq!(
            names[0].attributes(),
            &[NameAttribute::from("\\HasNoChildren")]
        );
        assert_eq!(names[0].delimiter(), Some("."));
        assert_eq!(names[0].name(), "INBOX");
    }

    #[test]
    fn parse_capabilities_w_unilateral() {
        let expected_capabilities = vec!["IMAP4rev1", "STARTTLS", "AUTH=GSSAPI", "LOGINDISABLED"];
        let lines = b"\
                    * CAPABILITY IMAP4rev1 STARTTLS AUTH=GSSAPI LOGINDISABLED\r\n\
                    * STATUS dev.github (MESSAGES 10 UIDNEXT 11 UIDVALIDITY 1408806928 UNSEEN 0)\r\n\
                    * 4 EXISTS\r\n";
        let (mut send, recv) = mpsc::channel();
        let capabilities = parse_capabilities(lines.to_vec(), &mut send).unwrap();

        assert_eq!(capabilities.len(), 4);
        for e in expected_capabilities {
            assert!(capabilities.has_str(e));
        }

        assert_eq!(
            recv.try_recv().unwrap(),
            UnsolicitedResponse::Status {
                mailbox: "dev.github".to_string(),
                attributes: vec![
                    StatusAttribute::Messages(10),
                    StatusAttribute::UidNext(11),
                    StatusAttribute::UidValidity(1408806928),
                    StatusAttribute::Unseen(0)
                ]
            }
        );
        assert_eq!(recv.try_recv().unwrap(), UnsolicitedResponse::Exists(4));
    }

    #[test]
    fn parse_ids_w_unilateral() {
        let lines = b"\
            * SEARCH 23 42 4711\r\n\
            * 1 RECENT\r\n\
            * STATUS INBOX (MESSAGES 10 UIDNEXT 11 UIDVALIDITY 1408806928 UNSEEN 0)\r\n";
        let (mut send, recv) = mpsc::channel();
        let ids = parse_ids(lines, &mut send).unwrap();

        assert_eq!(ids, [23, 42, 4711].iter().cloned().collect());

        assert_eq!(recv.try_recv().unwrap(), UnsolicitedResponse::Recent(1));
        assert_eq!(
            recv.try_recv().unwrap(),
            UnsolicitedResponse::Status {
                mailbox: "INBOX".to_string(),
                attributes: vec![
                    StatusAttribute::Messages(10),
                    StatusAttribute::UidNext(11),
                    StatusAttribute::UidValidity(1408806928),
                    StatusAttribute::Unseen(0)
                ]
            }
        );
    }

    #[test]
    fn parse_ids_test() {
        let lines = b"* SEARCH 1600 1698 1739 1781 1795 1885 1891 1892 1893 1898 1899 1901 1911 1926 1932 1933 1993 1994 2007 2032 2033 2041 2053 2062 2063 2065 2066 2072 2078 2079 2082 2084 2095 2100 2101 2102 2103 2104 2107 2116 2120 2135 2138 2154 2163 2168 2172 2189 2193 2198 2199 2205 2212 2213 2221 2227 2267 2275 2276 2295 2300 2328 2330 2332 2333 2334\r\n\
            * SEARCH 2335 2336 2337 2338 2339 2341 2342 2347 2349 2350 2358 2359 2362 2369 2371 2372 2373 2374 2375 2376 2377 2378 2379 2380 2381 2382 2383 2384 2385 2386 2390 2392 2397 2400 2401 2403 2405 2409 2411 2414 2417 2419 2420 2424 2426 2428 2439 2454 2456 2467 2468 2469 2490 2515 2519 2520 2521\r\n";
        let (mut send, recv) = mpsc::channel();
        let ids = parse_ids(lines, &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        let ids: HashSet<u32> = ids.iter().cloned().collect();
        assert_eq!(
            ids,
            [
                1600, 1698, 1739, 1781, 1795, 1885, 1891, 1892, 1893, 1898, 1899, 1901, 1911, 1926,
                1932, 1933, 1993, 1994, 2007, 2032, 2033, 2041, 2053, 2062, 2063, 2065, 2066, 2072,
                2078, 2079, 2082, 2084, 2095, 2100, 2101, 2102, 2103, 2104, 2107, 2116, 2120, 2135,
                2138, 2154, 2163, 2168, 2172, 2189, 2193, 2198, 2199, 2205, 2212, 2213, 2221, 2227,
                2267, 2275, 2276, 2295, 2300, 2328, 2330, 2332, 2333, 2334, 2335, 2336, 2337, 2338,
                2339, 2341, 2342, 2347, 2349, 2350, 2358, 2359, 2362, 2369, 2371, 2372, 2373, 2374,
                2375, 2376, 2377, 2378, 2379, 2380, 2381, 2382, 2383, 2384, 2385, 2386, 2390, 2392,
                2397, 2400, 2401, 2403, 2405, 2409, 2411, 2414, 2417, 2419, 2420, 2424, 2426, 2428,
                2439, 2454, 2456, 2467, 2468, 2469, 2490, 2515, 2519, 2520, 2521
            ]
            .iter()
            .cloned()
            .collect()
        );

        let lines = b"* SEARCH\r\n";
        let (mut send, recv) = mpsc::channel();
        let ids = parse_ids(lines, &mut send).unwrap();
        assert!(recv.try_recv().is_err());
        let ids: HashSet<u32> = ids.iter().cloned().collect();
        assert_eq!(ids, HashSet::<u32>::new());
    }
}
