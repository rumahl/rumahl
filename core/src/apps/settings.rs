//! Settings an app declares in its manifest.
//!
//! The signed manifest is the trusted source of the *shape* of an app's
//! settings: a stable key, a human title, an optional description and a typed
//! kind. rumahl renders this directly and never guesses a type. Stored values
//! and any extra configuration discovered at runtime are separate concerns.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

/// Maximum number of settings one manifest may declare.
pub const MAX_APP_SETTINGS: usize = 64;
const MAX_KEY_BYTES: usize = 64;
const MAX_TITLE_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_OPTIONS: usize = 64;
const MAX_OPTION_VALUE_BYTES: usize = 128;
const MAX_OPTION_LABEL_CHARS: usize = 120;

/// A stable, machine-readable setting key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppSettingKey(String);

impl AppSettingKey {
    pub fn parse(value: &str) -> Result<Self, AppSettingError> {
        if value.is_empty() {
            return Err(AppSettingError::EmptyKey);
        }
        if value.len() > MAX_KEY_BYTES {
            return Err(AppSettingError::KeyTooLong);
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        }) {
            return Err(AppSettingError::InvalidKey);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One choice of a [`AppSettingKind::Select`] setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingOption {
    value: String,
    label: String,
}

impl AppSettingOption {
    pub fn new(
        value: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Self, AppSettingError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_OPTION_VALUE_BYTES {
            return Err(AppSettingError::InvalidOptionValue);
        }
        let label = label.into();
        let label = label.trim();
        if label.is_empty() || label.chars().count() > MAX_OPTION_LABEL_CHARS {
            return Err(AppSettingError::InvalidOptionLabel);
        }
        Ok(Self {
            value,
            label: label.to_owned(),
        })
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

/// The declared type of a setting, which fixes how rumahl renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSettingKind {
    Text,
    Number,
    Boolean,
    Select { options: Vec<AppSettingOption> },
}

impl AppSettingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Select { .. } => "select",
        }
    }
}

/// A typed setting declared by an app manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettingDeclaration {
    key: AppSettingKey,
    title: String,
    description: Option<String>,
    kind: AppSettingKind,
    required: bool,
}

impl AppSettingDeclaration {
    pub fn new(
        key: AppSettingKey,
        title: impl Into<String>,
        kind: AppSettingKind,
    ) -> Result<Self, AppSettingError> {
        let title = title.into();
        let title = title.trim();
        if title.is_empty() {
            return Err(AppSettingError::EmptyTitle);
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(AppSettingError::TitleTooLong);
        }
        if let AppSettingKind::Select { options } = &kind {
            if options.is_empty() {
                return Err(AppSettingError::SelectRequiresOptions);
            }
            if options.len() > MAX_OPTIONS {
                return Err(AppSettingError::TooManyOptions);
            }
            let mut seen = HashSet::new();
            for option in options {
                if !seen.insert(option.value()) {
                    return Err(AppSettingError::DuplicateOption);
                }
            }
        }
        Ok(Self {
            key,
            title: title.to_owned(),
            description: None,
            kind,
            required: false,
        })
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, AppSettingError> {
        let description = description.into();
        let description = description.trim();
        if description.is_empty() {
            return Err(AppSettingError::EmptyDescription);
        }
        if description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(AppSettingError::DescriptionTooLong);
        }
        self.description = Some(description.to_owned());
        Ok(self)
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn key(&self) -> &AppSettingKey {
        &self.key
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn kind(&self) -> &AppSettingKind {
        &self.kind
    }

    pub fn is_required(&self) -> bool {
        self.required
    }
}

impl fmt::Display for AppSettingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSettingError {
    EmptyKey,
    KeyTooLong,
    InvalidKey,
    EmptyTitle,
    TitleTooLong,
    EmptyDescription,
    DescriptionTooLong,
    TooManyOptions,
    DuplicateOption,
    SelectRequiresOptions,
    InvalidOptionValue,
    InvalidOptionLabel,
}

impl fmt::Display for AppSettingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => f.write_str("app setting key cannot be empty"),
            Self::KeyTooLong => f.write_str("app setting key is too long"),
            Self::InvalidKey => f.write_str("app setting key contains invalid characters"),
            Self::EmptyTitle => f.write_str("app setting title cannot be empty"),
            Self::TitleTooLong => f.write_str("app setting title is too long"),
            Self::EmptyDescription => f.write_str("app setting description cannot be empty"),
            Self::DescriptionTooLong => f.write_str("app setting description is too long"),
            Self::TooManyOptions => f.write_str("app select setting declares too many options"),
            Self::DuplicateOption => f.write_str("app select setting has duplicate option values"),
            Self::SelectRequiresOptions => {
                f.write_str("app select setting must declare at least one option")
            }
            Self::InvalidOptionValue => f.write_str("app setting option value is invalid"),
            Self::InvalidOptionLabel => f.write_str("app setting option label is invalid"),
        }
    }
}

impl Error for AppSettingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: &str) -> AppSettingKey {
        AppSettingKey::parse(value).unwrap()
    }

    #[test]
    fn keys_are_bounded_and_lowercase_only() {
        assert_eq!(key("server.url").as_str(), "server.url");
        assert_eq!(
            AppSettingKey::parse("").unwrap_err(),
            AppSettingError::EmptyKey
        );
        assert_eq!(
            AppSettingKey::parse("Server.URL").unwrap_err(),
            AppSettingError::InvalidKey
        );
        assert_eq!(
            AppSettingKey::parse(&"a".repeat(65)).unwrap_err(),
            AppSettingError::KeyTooLong
        );
    }

    #[test]
    fn declarations_carry_title_description_and_type() {
        let setting =
            AppSettingDeclaration::new(key("server.url"), "  Server URL  ", AppSettingKind::Text)
                .unwrap()
                .with_description("Where the app reaches its backend.")
                .unwrap();

        assert_eq!(setting.key().as_str(), "server.url");
        assert_eq!(setting.title(), "Server URL");
        assert_eq!(
            setting.description(),
            Some("Where the app reaches its backend.")
        );
        assert_eq!(setting.kind().as_str(), "text");
        assert!(!setting.is_required());
    }

    #[test]
    fn rejects_empty_title_and_description() {
        assert_eq!(
            AppSettingDeclaration::new(key("a"), "  ", AppSettingKind::Text).unwrap_err(),
            AppSettingError::EmptyTitle
        );
        assert_eq!(
            AppSettingDeclaration::new(key("a"), "A", AppSettingKind::Boolean)
                .unwrap()
                .with_description("   ")
                .unwrap_err(),
            AppSettingError::EmptyDescription
        );
    }

    #[test]
    fn select_requires_unique_non_empty_options() {
        assert_eq!(
            AppSettingDeclaration::new(key("a"), "A", AppSettingKind::Select { options: vec![] })
                .unwrap_err(),
            AppSettingError::SelectRequiresOptions
        );

        let options = vec![
            AppSettingOption::new("a", "Alpha").unwrap(),
            AppSettingOption::new("a", "Again").unwrap(),
        ];
        assert_eq!(
            AppSettingDeclaration::new(key("a"), "A", AppSettingKind::Select { options })
                .unwrap_err(),
            AppSettingError::DuplicateOption
        );

        let options = vec![
            AppSettingOption::new("a", "Alpha").unwrap(),
            AppSettingOption::new("b", "Bravo").unwrap(),
        ];
        let setting =
            AppSettingDeclaration::new(key("a"), "A", AppSettingKind::Select { options }).unwrap();
        assert_eq!(setting.kind().as_str(), "select");
    }

    #[test]
    fn number_and_boolean_kinds_report_their_type() {
        assert_eq!(
            AppSettingDeclaration::new(key("retries"), "Retries", AppSettingKind::Number)
                .unwrap()
                .kind()
                .as_str(),
            "number"
        );
        assert_eq!(
            AppSettingDeclaration::new(key("enabled"), "Enabled", AppSettingKind::Boolean)
                .unwrap()
                .kind()
                .as_str(),
            "boolean"
        );
    }
}
