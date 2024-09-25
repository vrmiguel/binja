use std::collections::HashMap;

use aho_corasick::AhoCorasick;
use compact_str::CompactString as SmallStr;

pub type LanguageId = usize;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Duplicated key `{0}`")]
    DuplicatedKey(SmallStr),
    #[error("Unknown language key: `{0}`")]
    UnknownLanguage(SmallStr),
    #[error("Unknown argument: `{0}`")]
    UnknownArgument(SmallStr),
    #[error("Key not found: `{0}`")]
    MissingKey(SmallStr),
    #[error("Language not found: `{0}`")]
    MissingLanguage(SmallStr),
}

pub struct Translator {
    /// Every supported language in this Translator.
    /// Translations must be provided for all of the entries in this slice.
    languages: Box<[SmallStr]>,
    /// Maps each key to its [`Translation`].
    translations: HashMap<SmallStr, Translation>,
}

struct Translation {
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
        // TODO: make generic
        key: S3,
        arguments: I1,
        translations: I2,
    ) -> Result<(), Error> {
        let key = key.into();
        if self.translations.contains_key(&key) {
            return Err(Error::DuplicatedKey(key.clone()));
        }

        let arguments: Box<[SmallStr]> = arguments.into_iter().map(Into::into).collect();

        let mut processed_translations = HashMap::with_capacity(self.languages.len());

        for (language_key, message) in translations {
            // TODO: check if we can change this to AsRef<str>
            let language_key: SmallStr = language_key.into();
            let language_id = self
                .languages
                .iter()
                .position(|lang| lang == language_key)
                .ok_or(Error::UnknownLanguage(language_key))?;

            processed_translations.insert(language_id, message.into());
        }

        let translation = Translation {
            arguments,
            translations: processed_translations,
        };

        // TODO: Check if we have enough translations
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
            .ok_or_else(|| Error::MissingKey(language.into()))?;
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
                .ok_or_else(|| Error::UnknownLanguage(argument_received.clone()))?;

            if arguments.contains(&argument_received) {
                panic!("Duplicated argument")
            } else {
                arguments.push(argument_received);
                values_to_replace.push(value_to_replace.into());
            }
        }

        // TODO: cache AhoCorasick automatons, or store them directly instead of Strings
        let ac = AhoCorasick::new(arguments).unwrap();

        Ok(ac
            .try_replace_all(&message_to_translate, &values_to_replace)
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, Translator};

    #[test]
    fn igorcafe()  -> Result<(), Error> {
        let mut tr = Translator::new(["pt", "en"]);

        tr.add_text(
            "greetings",
            ["NAME"],
            [
                ("pt", "NAME rebolou gostoso pros cria"),
                ("en", "NAME danced really well for the kids."),
            ],
        )?;
    
        let translated = tr.translate("greetings", "pt", [("NAME", "igorcafe")])?;
        dbg!(&translated);
    
        assert_eq!(translated, "igorcafe rebolou gostoso pros cria");
    
        Ok(())
    }
}
