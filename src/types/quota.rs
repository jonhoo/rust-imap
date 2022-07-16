use crate::error::{Error, ParseError};
use crate::parse::{parse_many_into, try_handle_unilateral, MapOrNot};
use crate::types::UnsolicitedResponse;
use imap_proto::Response;
use ouroboros::self_referencing;
use std::fmt::{Display, Formatter};
use std::sync::mpsc;

pub struct QuotaLimit {
    // The resource type
    pub name: QuotaLimitName,
    // The amount for that resource
    pub amount: u64,
}

impl Display for QuotaLimit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.amount)
    }
}

/// https://datatracker.ietf.org/doc/html/rfc2087#section-3
pub enum QuotaLimitName {
    /// Sum of messages' RFC822.SIZE, in units of 1024 octets
    Storage,
    /// Number of messages
    Message,
    /// Any other string (for future RFCs)
    Atom(String),
}

impl Display for QuotaLimitName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaLimitName::Storage => write!(f, "STORAGE"),
            QuotaLimitName::Message => write!(f, "MESSAGE"),
            QuotaLimitName::Atom(s) => write!(f, "{}", s),
        }
    }
}

#[self_referencing]
pub struct Quota {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) quota: imap_proto::Quota<'this>,
}

impl Quota {
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        QuotaTryBuilder {
            data: owned,
            quota_builder: |input| {
                let mut lines: &[u8] = input;

                // There should only be one single QUOTA response
                while !lines.is_empty() {
                    match imap_proto::parser::parse_response(lines) {
                        Ok((_rest, Response::Quota(a))) => {
                            return Ok(a);
                        }
                        Ok((rest, data)) => {
                            lines = rest;
                            if let Some(resp) = try_handle_unilateral(data, unsolicited) {
                                return Err(resp.into());
                            }
                        }
                        _ => {
                            return Err(Error::Parse(ParseError::Invalid(lines.to_vec())));
                        }
                    }
                }

                Err(Error::Parse(ParseError::Invalid(lines.to_vec())))
            },
        }
        .try_build()
    }

    pub fn root_name(&self) -> &str {
        &*self.borrow_quota().root_name
    }

    pub fn resources(&self) -> &[imap_proto::QuotaResource<'_>] {
        &*self.borrow_quota().resources
    }
}

#[self_referencing]
pub struct QuotaRoot {
    data: Vec<u8>,
    #[borrows(data)]
    #[covariant]
    pub(crate) inner: InnerQuotaRoot<'this>,
}

pub(crate) struct InnerQuotaRoot<'a> {
    pub(crate) quota_root: imap_proto::QuotaRoot<'a>,
    pub(crate) quotas: Vec<imap_proto::Quota<'a>>,
}

impl QuotaRoot {
    pub fn parse(
        owned: Vec<u8>,
        unsolicited: &mut mpsc::Sender<UnsolicitedResponse>,
    ) -> Result<Self, Error> {
        QuotaRootTryBuilder {
            data: owned,
            inner_builder: |input| {
                let mut quota_roots: Vec<imap_proto::QuotaRoot<'_>> = Vec::new();
                let mut quotas = Vec::new();
                parse_many_into(
                    input,
                    &mut quota_roots,
                    unsolicited,
                    |response| match response {
                        Response::QuotaRoot(q) => Ok(MapOrNot::Map(q)),
                        Response::Quota(_) => Ok(MapOrNot::Ignore),
                        resp => Ok(MapOrNot::Not(resp)),
                    },
                )?;
                parse_many_into(input, &mut quotas, unsolicited, |response| match response {
                    Response::QuotaRoot(_) => Ok(MapOrNot::Ignore),
                    Response::Quota(q) => Ok(MapOrNot::Map(q)),
                    resp => Ok(MapOrNot::Not(resp)),
                })?;
                if quota_roots.is_empty() {
                    Err(Error::Parse(ParseError::Invalid(input.to_vec())))
                } else {
                    Ok(InnerQuotaRoot {
                        quota_root: quota_roots.first().unwrap().clone(),
                        quotas,
                    })
                }
            },
        }
        .try_build()
    }

    pub fn mailbox_name(&self) -> &str {
        &*self.borrow_inner().quota_root.mailbox_name
    }

    pub fn quota_root_names(&self) -> Vec<&str> {
        self.borrow_inner()
            .quota_root
            .quota_root_names
            .iter()
            .map(|e| &*e.as_ref())
            .collect()
    }

    pub fn quotas(&self) -> &[imap_proto::Quota<'_>] {
        &self.borrow_inner().quotas[..]
    }
}
