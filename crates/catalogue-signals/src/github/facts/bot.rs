//! Bot detection for commit and item authors.

/// The suffix GitHub gives the login of a bot account.
const BOT_SUFFIX: &str = "[bot]";

/// Logins and names, compared without case, that mark a bot without the suffix.
const KNOWN_BOTS: [&str; 3] = ["dependabot", "renovate", "github-actions"];

/// Whether `text` ends in the bot suffix, ignoring case.
fn has_suffix(text: &str) -> bool {
    text.len() >= BOT_SUFFIX.len()
        && text
            .get(text.len() - BOT_SUFFIX.len()..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(BOT_SUFFIX))
}

/// Whether `text` is one of [`KNOWN_BOTS`].
fn is_known(text: &str) -> bool {
    KNOWN_BOTS
        .iter()
        .any(|bot| text.trim().eq_ignore_ascii_case(bot))
}

/// Whether a commit author is a bot.
///
/// The author is a bot when the login ends in `[bot]`, when no user is linked
/// and the name ends in `[bot]`, or when the login or the name is a known bot.
pub(super) fn is_bot_commit(login: Option<&str>, name: Option<&str>) -> bool {
    let suffixed = match login {
        Some(login) => has_suffix(login),
        None => name.is_some_and(has_suffix),
    };
    suffixed || login.is_some_and(is_known) || name.is_some_and(is_known)
}

/// Whether the login of an issue or pull request author is a bot.
pub(super) fn is_bot_login(login: &str) -> bool {
    has_suffix(login) || is_known(login)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixed_logins_are_bots() {
        assert!(is_bot_commit(Some("dependabot[bot]"), Some("Alice")));
        assert!(is_bot_commit(Some("Foo[BOT]"), None));
    }

    #[test]
    fn a_suffixed_name_counts_only_without_a_user() {
        assert!(is_bot_commit(None, Some("release-plz[bot]")));
        assert!(!is_bot_commit(Some("alice"), Some("release-plz[bot]")));
    }

    #[test]
    fn known_bots_match_by_login_or_name_without_case() {
        assert!(is_bot_commit(Some("Renovate"), None));
        assert!(is_bot_commit(None, Some("GitHub-Actions")));
        assert!(is_bot_commit(Some("alice"), Some("dependabot")));
    }

    #[test]
    fn people_are_not_bots() {
        assert!(!is_bot_commit(Some("alice"), Some("Alice")));
        assert!(!is_bot_commit(None, None));
        assert!(!is_bot_commit(None, Some("Bob")));
        assert!(!is_bot_login("robot"));
        assert!(is_bot_login("renovate"));
        assert!(is_bot_login("x[bot]"));
    }

    #[test]
    fn short_text_is_safe() {
        assert!(!has_suffix("ot]"));
        assert!(!has_suffix("é"));
    }
}
