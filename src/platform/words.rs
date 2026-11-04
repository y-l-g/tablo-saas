//! What the app says outside a request.
//!
//! A panel's catalog answers `tablo::t!` only through the router's context, which a job never
//! holds: there `t!` renders the message's id, and the mail still goes out. So a job's words come
//! from here instead, parsed from the same files the panels load.

use std::sync::LazyLock;

use topcoat::l10n::{Argument, Catalog, Locale, locale};

pub const ENGLISH: &str = include_str!("../../locales/en.ftl");
pub const FRENCH: &str = include_str!("../../locales/fr.ftl");

/// The language of a job's words, the panels' default: a job has no browser to ask.
const DEFAULT_LOCALE: Locale = locale!("en");

/// What Fluent wraps an argument in. A browser reads them; a mail subject shows them to some
/// clients, and they break a search for the team's name.
const BIDI_ISOLATES: [char; 2] = ['\u{2068}', '\u{2069}'];

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    Catalog::new([
        (locale!("en"), &[ENGLISH][..]),
        (locale!("fr"), &[FRENCH][..]),
    ])
});

/// The message `id` in the default language. An id with no message is an error, not text: it
/// would go out as the subject of a mail.
pub fn job_text(id: &str, arguments: &[(&str, Argument<'_>)]) -> anyhow::Result<String> {
    in_language(&DEFAULT_LOCALE, id, arguments)
}

fn in_language(
    locale: &Locale,
    id: &str,
    arguments: &[(&str, Argument<'_>)],
) -> anyhow::Result<String> {
    CATALOG
        .format(locale, id, None, arguments)
        .map(|text| text.replace(BIDI_ISOLATES, ""))
        .ok_or_else(|| anyhow::anyhow!("no message `{id}` for {locale}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids a job can reach for, which live in strings a compiler cannot check.
    fn mail_ids(source: &str) -> Vec<&str> {
        source
            .lines()
            .filter_map(|line| line.split_once(" ="))
            .map(|(id, _)| id)
            .filter(|id| id.starts_with("mail-"))
            .collect()
    }

    #[test]
    fn both_languages_hold_the_same_mail() {
        let mut english = mail_ids(ENGLISH);
        let mut french = mail_ids(FRENCH);
        english.sort_unstable();
        french.sort_unstable();
        assert!(english.contains(&"mail-invitation-subject"));
        assert_eq!(english, french);
    }

    #[test]
    fn a_message_reads_its_arguments() {
        let text = job_text(
            "mail-invitation-subject",
            &[("team", Argument::from("Acme"))],
        )
        .expect("the message exists");
        assert_eq!(text, "Join Acme");
    }

    #[test]
    fn a_missing_message_is_an_error() {
        assert!(job_text("mail-nothing-by-this-name", &[]).is_err());
    }
}
