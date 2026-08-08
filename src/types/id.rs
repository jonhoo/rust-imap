use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_until};
use nom::character::complete::{char, multispace0, multispace1};
use nom::combinator::{map, value};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded};
use nom::IResult;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

/// From [ID Response](https://datatracker.ietf.org/doc/html/rfc2971#section-3.2)
///
/// Used by [`Session::id`](crate::Session::id)
#[derive(Debug, Clone)]
pub struct IdResponse {
    /// Fields of the response
    pub fields: HashMap<Vec<u8>, Option<Vec<u8>>>,
}

impl IdResponse {
    /// Parse from the server raw response
    pub fn parse(data: &[u8]) -> Self {
        let mut parser = preceded(
            alt((tag("* ID "), tag_no_case("* ID "))),
            alt((
                value(HashMap::new(), tag_no_case("NIL")), /* The whole list is a NIL */
                delimited(
                    char('('),
                    map(separated_list0(multispace1, pair_parser), |vec| {
                        vec.into_iter().collect::<HashMap<_, _>>()
                    }),
                    char(')'),
                ),
            )),
        );

        match parser(data) {
            Ok((_, fields)) => Self { fields },
            Err(_) => Self {
                fields: HashMap::new(),
            },
        }
    }

    /// Get field as UTF-8
    pub fn get(&self, key: &[u8]) -> Option<String> {
        self.fields
            .get(key)
            .and_then(|x| x.as_ref().map(|x| String::from_utf8_lossy(x).into_owned()))
    }

    /// Field length
    pub fn len(&self) -> usize {
        self.fields.len()
    }
}

/// Parse quoted string
fn quoted_string(input: &[u8]) -> IResult<&[u8], Vec<u8>> {
    map(
        delimited(char('"'), take_until("\""), char('"')),
        |s: &[u8]| s.to_vec(),
    )(input)
}

/// Parse 'nstring': "string" or NIL
fn nstring(input: &[u8]) -> IResult<&[u8], Option<Vec<u8>>> {
    alt((map(quoted_string, Some), value(None, tag_no_case("NIL"))))(input)
}

/// Parse key-value pair: "name" "value"
fn pair_parser(input: &[u8]) -> IResult<&[u8], (Vec<u8>, Option<Vec<u8>>)> {
    let (input, key) = preceded(multispace0, quoted_string)(input)?;
    let (input, val) = preceded(multispace1, nstring)(input)?;
    Ok((input, (key, val)))
}

impl Display for IdResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.fields {
            let key_str = String::from_utf8_lossy(k);
            let val_str = v
                .as_ref()
                .map(|b| format!("\"{}\"", String::from_utf8_lossy(b)))
                .unwrap_or_else(|| "NIL".to_string());
            write!(f, "{}={}; ", key_str, val_str)?;
        }
        Ok(())
    }
}
