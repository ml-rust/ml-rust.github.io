/// Parses `bytes <start>-<end>/<total>` into the start and the total.
pub(super) fn parse_content_range(value: &str) -> Option<(u64, u64)> {
    let range = value.trim().strip_prefix("bytes ")?;
    let (span, total) = range.split_once('/')?;
    let (start, _end) = span.split_once('-')?;
    Some((start.parse().ok()?, total.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_range_gives_start_and_total() {
        assert_eq!(parse_content_range("bytes 100-199/200"), Some((100, 200)));
    }

    #[test]
    fn content_range_rejects_unknown_total_and_other_units() {
        assert_eq!(parse_content_range("bytes 100-199/*"), None);
        assert_eq!(parse_content_range("items 1-2/3"), None);
    }
}
