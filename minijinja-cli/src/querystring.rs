//! A small parser for URL encoded query strings.
//!
//! Supports bracket notation for nested values similar to what PHP/Rails
//! (and `serde_qs`) support:
//!
//! - `a[b]=1&a[c]=2` creates a nested map
//! - `a[0]=x&a[1]=y` creates a list (ordered by index)
//! - `a[]=x&a[]=y` creates a list (in insertion order)
use std::collections::BTreeMap;

use anyhow::{bail, Error};
use minijinja::Value;

enum Level {
    Nested(BTreeMap<String, Level>),
    OrderedSeq(BTreeMap<usize, Level>),
    Sequence(Vec<Level>),
    Flat(String),
}

impl Level {
    fn into_value(self) -> Value {
        match self {
            Level::Nested(map) => Value::from(
                map.into_iter()
                    .map(|(k, v)| (k, v.into_value()))
                    .collect::<BTreeMap<_, _>>(),
            ),
            Level::OrderedSeq(map) => map.into_values().map(Level::into_value).collect(),
            Level::Sequence(seq) => seq.into_iter().map(Level::into_value).collect(),
            Level::Flat(s) => Value::from(s),
        }
    }
}

/// Parses a query string into a map.
pub fn from_bytes(input: &[u8]) -> Result<BTreeMap<String, Value>, Error> {
    let mut root = BTreeMap::new();
    for pair in input.split(|&c| c == b'&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.iter().position(|&c| c == b'=') {
            Some(idx) => (&pair[..idx], &pair[idx + 1..]),
            None => (pair, &b""[..]),
        };
        let value = decode(value)?;
        let mut segments = split_key(key)?.into_iter();
        let first = match segments.next() {
            Some(first) if !first.is_empty() => first,
            _ => continue,
        };
        insert(&mut root, first, segments.collect(), value)?;
    }
    Ok(root.into_iter().map(|(k, v)| (k, v.into_value())).collect())
}

fn insert(
    map: &mut BTreeMap<String, Level>,
    key: String,
    rest: Vec<String>,
    value: String,
) -> Result<(), Error> {
    if rest.is_empty() {
        if map.contains_key(&key) {
            bail!("Multiple values for one key: {:?}", key);
        }
        map.insert(key, Level::Flat(value));
        return Ok(());
    }
    let level = map
        .entry(key.clone())
        .or_insert_with(|| new_level(&rest[0]));
    insert_into_level(level, &key, rest, value)
}

fn new_level(segment: &str) -> Level {
    if segment.is_empty() {
        Level::Sequence(Vec::new())
    } else if segment.parse::<usize>().is_ok() {
        Level::OrderedSeq(BTreeMap::new())
    } else {
        Level::Nested(BTreeMap::new())
    }
}

fn insert_into_level(
    level: &mut Level,
    key: &str,
    mut rest: Vec<String>,
    value: String,
) -> Result<(), Error> {
    let segment = rest.remove(0);
    match level {
        Level::Sequence(seq) if segment.is_empty() => {
            if rest.is_empty() {
                seq.push(Level::Flat(value));
            } else {
                let mut child = new_level(&rest[0]);
                insert_into_level(&mut child, key, rest, value)?;
                seq.push(child);
            }
            Ok(())
        }
        Level::OrderedSeq(map) if segment.parse::<usize>().is_ok() => {
            let idx = segment.parse::<usize>().unwrap();
            if rest.is_empty() {
                if map.contains_key(&idx) {
                    bail!(
                        "Multiple values for one key: {:?}",
                        format!("{}[{}]", key, idx)
                    );
                }
                map.insert(idx, Level::Flat(value));
                Ok(())
            } else {
                let child = map.entry(idx).or_insert_with(|| new_level(&rest[0]));
                insert_into_level(child, key, rest, value)
            }
        }
        Level::Nested(map) if !segment.is_empty() && segment.parse::<usize>().is_err() => {
            insert(map, segment, rest, value)
        }
        Level::Flat(_) => bail!("Multiple values for one key: {:?}", key),
        _ => bail!("Mixed key types for key {:?}", key),
    }
}

/// Splits `a[b][c]` into `["a", "b", "c"]`.  If the key is not well formed
/// bracket notation, it's used as a literal key.
fn split_key(key: &[u8]) -> Result<Vec<String>, Error> {
    let literal = || Ok(vec![decode(key)?]);
    let Some(open) = key.iter().position(|&c| c == b'[') else {
        return literal();
    };
    let mut rv = vec![decode(&key[..open])?];
    let mut rest = &key[open..];
    while !rest.is_empty() {
        if rest[0] != b'[' {
            return literal();
        }
        let Some(close) = rest.iter().position(|&c| c == b']') else {
            return literal();
        };
        rv.push(decode(&rest[1..close])?);
        rest = &rest[close + 1..];
    }
    Ok(rv)
}

/// Decodes `+` and percent escapes.  Invalid escapes are left as-is.
fn decode(s: &[u8]) -> Result<String, Error> {
    fn hex(c: u8) -> Option<u8> {
        (c as char).to_digit(16).map(|x| x as u8)
    }

    let mut rv = Vec::with_capacity(s.len());
    let mut idx = 0;
    while idx < s.len() {
        let c = s[idx];
        idx += 1;
        if c == b'+' {
            rv.push(b' ');
        } else if c == b'%' {
            let hi = s.get(idx).and_then(|&c| hex(c));
            let lo = s.get(idx + 1).and_then(|&c| hex(c));
            if let (Some(hi), Some(lo)) = (hi, lo) {
                rv.push(hi << 4 | lo);
                idx += 2;
            } else {
                rv.push(b'%');
            }
        } else {
            rv.push(c);
        }
    }
    Ok(String::from_utf8(rv)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> String {
        match from_bytes(s.as_bytes()) {
            Ok(value) => serde_json::to_string(&value).unwrap(),
            Err(err) => format!("error: {}", err),
        }
    }

    #[test]
    fn test_basics() {
        assert_eq!(parse(""), "{}");
        assert_eq!(parse("a=1&b=x+y"), r#"{"a":"1","b":"x y"}"#);
        assert_eq!(parse("a&b="), r#"{"a":"","b":""}"#);
        assert_eq!(parse("a=1&&b=2"), r#"{"a":"1","b":"2"}"#);
        assert_eq!(parse("a=%C3%A4%2f"), r#"{"a":"ä/"}"#);
        assert_eq!(parse("a=%ZZ%2"), r#"{"a":"%ZZ%2"}"#);
        assert_eq!(parse("a%5Bb%5D=1"), r#"{"a[b]":"1"}"#);
        assert_eq!(
            parse("a=1&a=2"),
            r#"error: Multiple values for one key: "a""#
        );
        assert_eq!(
            parse("a=%FF"),
            "error: invalid utf-8 sequence of 1 bytes from index 0"
        );
    }

    #[test]
    fn test_nested() {
        assert_eq!(parse("a[b]=1&a[c]=2"), r#"{"a":{"b":"1","c":"2"}}"#);
        assert_eq!(parse("a[b][c]=d"), r#"{"a":{"b":{"c":"d"}}}"#);
        assert_eq!(parse("a[1]=x&a[0]=y"), r#"{"a":["y","x"]}"#);
        assert_eq!(parse("a[]=1&a[]=2"), r#"{"a":["1","2"]}"#);
        assert_eq!(
            parse("a[0][b]=1&a[0][c]=2&a[1][b]=3"),
            r#"{"a":[{"b":"1","c":"2"},{"b":"3"}]}"#
        );
        assert_eq!(
            parse("a[b]=1&a=2"),
            r#"error: Multiple values for one key: "a""#
        );
        assert_eq!(
            parse("a[b]=1&a[0]=2"),
            r#"error: Mixed key types for key "a""#
        );
        assert_eq!(parse("a[b=1"), r#"{"a[b":"1"}"#);
    }
}
