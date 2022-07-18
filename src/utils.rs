/// Lovingly borrowed from the cargo crate
///
/// Joins an iterator of [std::fmt::Display]'ables into an output writable
pub(crate) fn iter_join_onto<W, I, T>(mut w: W, iter: I, delim: &str) -> std::fmt::Result
where
    W: std::fmt::Write,
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    let mut it = iter.into_iter().peekable();
    while let Some(n) = it.next() {
        write!(w, "{}", n)?;
        if it.peek().is_some() {
            write!(w, "{}", delim)?;
        }
    }
    Ok(())
}

/// Lovingly borrowed from the cargo crate
///
/// Joins an iterator of [std::fmt::Display]'ables to a new [std::string::String].
pub(crate) fn iter_join<I, T>(iter: I, delim: &str) -> String
where
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    let mut s = String::new();
    let _ = iter_join_onto(&mut s, iter, delim);
    s
}
