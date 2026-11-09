//! The app's words: the catalogs the panels load, which a job reads too.
//!
//! A job's context comes from the panel's handle, which holds the panel's catalogs, so `tablo::t!`
//! reads them there as in a request. It reads the panels' default language until the job names
//! the reader's with [`for_reader`].

use topcoat::{context::Cx, l10n::Locale};

pub const ENGLISH: &str = include_str!("../../locales/en.ftl");
pub const FRENCH: &str = include_str!("../../locales/fr.ftl");

/// `cx`, a job's, writing in `locale`, the reader's, or in the panels' default without one.
pub fn for_reader(cx: &Cx, locale: Option<Locale>) -> Cx {
    match locale {
        Some(locale) => cx.with(locale),
        None => cx.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids a job can reach for, which live in strings a compiler cannot check: a missing one
    /// would go out as the subject of a mail.
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
}
