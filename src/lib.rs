use std::collections::HashMap;

use aho_corasick::AhoCorasick;
use compact_str::CompactString as SmallStr;

pub type LanguageId = usize;

#[cfg_attr(test, derive(PartialEq))]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Duplicated key `{0}`")]
    DuplicatedKey(SmallStr),
    #[error("Duplicated argument `{0}`")]
    DuplicatedArgument(SmallStr),
    #[error("Unknown language key: `{0}`")]
    UnknownLanguage(SmallStr),
    #[error("Unknown argument: `{0}`")]
    UnknownArgument(SmallStr),
    #[error("Key not found: `{0}`")]
    MissingKey(SmallStr),
    #[error("Language not found: `{0}`")]
    MissingLanguage(SmallStr),
    #[error("Replacement error: `{0}`")]
    AhoCorasickMatch(#[from] aho_corasick::MatchError),
    // Note: this is a stringified version of `aho_corasick::MatchError` since it does not implement PartialEq
    #[error("Replacement error: `{0}`")]
    AhoCorasickBuild(String),
}

pub struct Translator {
    /// Every supported language in this Translator.
    /// Translations must be provided for all of the entries in this slice.
    languages: Box<[SmallStr]>,
    /// Maps each key to its [`Translation`].
    translations: HashMap<SmallStr, Translation>,
}

struct Translation {
    // TODO: store arguments in descending order
    /// Arguments to be inserted into the given phrase.
    arguments: Box<[SmallStr]>,
    // LanguageId refers to the index of the given language in `Translator::languages`.
    translations: HashMap<LanguageId, SmallStr>,
}

impl Translator {
    pub fn new<S: Into<SmallStr>, I: IntoIterator<Item = S>>(languages: I) -> Self {
        let mut languages: Vec<SmallStr> = languages.into_iter().map(Into::into).collect();

        // Handle possibly duplicated input
        languages.sort();
        languages.dedup();

        Self {
            languages: languages.into(),
            translations: Default::default(),
        }
    }

    pub fn add_text<
        S1: Into<SmallStr>,
        S2: Into<SmallStr>,
        S3: Into<SmallStr>,
        I1: IntoIterator<Item = S1>,
        I2: IntoIterator<Item = (S1, S2)>,
    >(
        &mut self,
        key: S3,
        arguments: I1,
        translations: I2,
    ) -> Result<(), Error> {
        let key = key.into();
        if self.translations.contains_key(&key) {
            return Err(Error::DuplicatedKey(key.clone()));
        }

        let arguments = arguments.into_iter().map(Into::into).collect();

        let mut processed_translations = HashMap::with_capacity(self.languages.len());

        for (language_key, message) in translations {
            // TODO: check if we can change this to AsRef<str>
            let language_key: SmallStr = language_key.into();
            let language_id = self
                .languages
                .iter()
                .position(|lang| lang == language_key)
                .ok_or_else(|| Error::UnknownLanguage(language_key.clone()))?;

            let is_duplicate = processed_translations
                .insert(language_id, message.into())
                .is_some();

            if is_duplicate {
                return Err(Error::DuplicatedKey(language_key));
            }
        }

        if processed_translations.len() < self.languages.len() {
            return Err(Error::MissingLanguage(
                "Not all languages have translations".into(),
            ));
        }

        let translation = Translation {
            arguments,
            translations: processed_translations,
        };

        // TODO: Check if we have duplicate translations
        self.translations.insert(key, translation);

        Ok(())
    }

    pub fn translate<S1: Into<SmallStr>, S2: Into<SmallStr>, I: IntoIterator<Item = (S1, S2)>>(
        &self,
        key: &str,
        language: &str,
        args: I,
    ) -> Result<String, Error> {
        // Fetch the appropriate translation based on key and language
        let translation = self
            .translations
            .get(key)
            .ok_or_else(|| Error::MissingKey(key.into()))?;

        let language_id = self
            .languages
            .iter()
            .position(|lang| *lang == language)
            .ok_or_else(|| Error::UnknownLanguage(language.into()))?;
        let message_to_translate = &translation.translations[&language_id];

        let mut arguments = Vec::new();
        let mut values_to_replace = Vec::new();

        for (argument_received, value_to_replace) in args {
            let argument_received = argument_received.into();

            // Check if we are expecting this argument
            translation
                .arguments
                .iter()
                .find(|arg| *arg == argument_received)
                .ok_or_else(|| Error::UnknownArgument(argument_received.clone()))?;

            if arguments.contains(&argument_received) {
                return Err(Error::DuplicatedArgument(argument_received));
            } else {
                arguments.push(argument_received);
                values_to_replace.push(value_to_replace.into());
            }
        }

        // TODO: cache AhoCorasick automatons, or store them directly instead of Strings
        let ac =
            AhoCorasick::new(arguments).map_err(|err| Error::AhoCorasickBuild(err.to_string()))?;

        ac.try_replace_all(message_to_translate, &values_to_replace)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, Translator};

    #[test]
    fn one_argument() -> Result<(), Error> {
        let mut tr = Translator::new(["pt", "en", "it"]);

        tr.add_text(
            "greetings",
            ["NAME"],
            [
                ("en", "Good morning, NAME!"),
                ("pt", "Bom dia, NAME!"),
                ("it", "Buongiorno, NAME!"),
            ],
        )?;

        assert_eq!(
            tr.translate("greetings", "pt", [("NAME", "Julian")])?,
            "Bom dia, Julian!"
        );
        assert_eq!(
            tr.translate("greetings", "en", [("NAME", "Julian")])?,
            "Good morning, Julian!"
        );
        assert_eq!(
            tr.translate("greetings", "it", [("NAME", "Julian")])?,
            "Buongiorno, Julian!"
        );

        // Validations
        assert_eq!(
            tr.translate("greetings", "cz", [("NAME", "Julian")]),
            Err(Error::UnknownLanguage("cz".into()))
        );
        assert_eq!(
            tr.translate("greetings", "pt", [("NOME", "Julian")]),
            Err(Error::UnknownArgument("NOME".into()))
        );

        Ok(())
    }

    #[test]
    fn overlapping_arguments() -> Result<(), Error> {
        let mut tr = Translator::new(["pt", "en", "it"]);

        tr.add_text(
            "greetings",
            ["NAME", "NAME2"],
            [
                ("en", "Good morning, NAME! Good afternoon, NAME2!"),
                ("pt", "Bom dia, NAME! Boa tarde, NAME2!"),
                ("it", "Buongiorno, NAME! Buon pomeriggio, NAME2!"),
            ],
        )?;

        // TODO: disallow this
        dbg!(
            tr.translate("greetings", "pt", [("NAME", "Julian"), ("NAME2", "Kyle")])?
        );

        Ok(())
    }
}
