use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::Deserialize;

use crate::{MANIFEST_VERSION, UI_CONTRACT_VERSION, WindowChromeVariant};

const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_THEME_ID_BYTES: usize = 128;
const MAX_THEME_NAME_CHARS: usize = 128;
const MAX_TOKENS: usize = 64;
const MAX_VARIANTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThemeToken {
    ColorCanvasBackground,
    ColorPanelBackground,
    ColorTextPrimary,
    ColorTextMuted,
    ColorAccentPrimary,
    ColorWindowTitlebarBackground,
    ColorWindowTitlebarForeground,
    RadiusWindow,
    SpaceShellGap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeTokenValue {
    Color(String),
    Pixels(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeManifest {
    id: String,
    name: String,
    tokens: BTreeMap<ThemeToken, ThemeTokenValue>,
    window_chrome: WindowChromeVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTheme {
    id: String,
    name: String,
    tokens: BTreeMap<ThemeToken, ThemeTokenValue>,
    window_chrome: WindowChromeVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeManifestError {
    TooLarge,
    InvalidJson,
    UnsupportedManifestVersion(u16),
    UnsupportedUiContractVersion(u16),
    InvalidId,
    InvalidName,
    TooManyTokens,
    TooManyVariants,
    UnknownToken(String),
    InvalidTokenValue(String),
    UnknownVariant(String),
    InvalidVariantValue(String),
    InsufficientContrast(&'static str),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireThemeManifest {
    manifest_version: u16,
    ui_contract_version: u16,
    id: String,
    name: String,
    #[serde(default)]
    tokens: BTreeMap<String, String>,
    #[serde(default)]
    variants: BTreeMap<String, String>,
}

impl ThemeToken {
    pub const ALL: [Self; 9] = [
        Self::ColorCanvasBackground,
        Self::ColorPanelBackground,
        Self::ColorTextPrimary,
        Self::ColorTextMuted,
        Self::ColorAccentPrimary,
        Self::ColorWindowTitlebarBackground,
        Self::ColorWindowTitlebarForeground,
        Self::RadiusWindow,
        Self::SpaceShellGap,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::ColorCanvasBackground => "color.canvas.background",
            Self::ColorPanelBackground => "color.panel.background",
            Self::ColorTextPrimary => "color.text.primary",
            Self::ColorTextMuted => "color.text.muted",
            Self::ColorAccentPrimary => "color.accent.primary",
            Self::ColorWindowTitlebarBackground => "color.window.titlebar.background",
            Self::ColorWindowTitlebarForeground => "color.window.titlebar.foreground",
            Self::RadiusWindow => "radius.window",
            Self::SpaceShellGap => "space.shell.gap",
        }
    }

    pub fn css_property(self) -> &'static str {
        match self {
            Self::ColorCanvasBackground => "--rumahl-color-canvas-background",
            Self::ColorPanelBackground => "--rumahl-color-panel-background",
            Self::ColorTextPrimary => "--rumahl-color-text-primary",
            Self::ColorTextMuted => "--rumahl-color-text-muted",
            Self::ColorAccentPrimary => "--rumahl-color-accent-primary",
            Self::ColorWindowTitlebarBackground => "--rumahl-color-window-titlebar-background",
            Self::ColorWindowTitlebarForeground => "--rumahl-color-window-titlebar-foreground",
            Self::RadiusWindow => "--rumahl-radius-window",
            Self::SpaceShellGap => "--rumahl-space-shell-gap",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|token| token.id() == value)
    }

    fn parse_value(self, value: &str) -> Option<ThemeTokenValue> {
        match self {
            Self::ColorCanvasBackground
            | Self::ColorPanelBackground
            | Self::ColorTextPrimary
            | Self::ColorTextMuted
            | Self::ColorAccentPrimary
            | Self::ColorWindowTitlebarBackground
            | Self::ColorWindowTitlebarForeground => parse_color(value),
            Self::RadiusWindow => parse_pixels(value, 32),
            Self::SpaceShellGap => parse_pixels(value, 64),
        }
    }
}

impl ThemeTokenValue {
    pub fn as_css_value(&self) -> String {
        match self {
            Self::Color(value) => value.clone(),
            Self::Pixels(value) => format!("{value}px"),
        }
    }
}

impl ThemeManifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self, ThemeManifestError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ThemeManifestError::TooLarge);
        }
        let wire: WireThemeManifest =
            serde_json::from_slice(bytes).map_err(|_| ThemeManifestError::InvalidJson)?;
        if wire.manifest_version != MANIFEST_VERSION {
            return Err(ThemeManifestError::UnsupportedManifestVersion(
                wire.manifest_version,
            ));
        }
        if wire.ui_contract_version != UI_CONTRACT_VERSION {
            return Err(ThemeManifestError::UnsupportedUiContractVersion(
                wire.ui_contract_version,
            ));
        }
        if !valid_namespaced_id(&wire.id) {
            return Err(ThemeManifestError::InvalidId);
        }
        let name = wire.name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_THEME_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(ThemeManifestError::InvalidName);
        }
        if wire.tokens.len() > MAX_TOKENS {
            return Err(ThemeManifestError::TooManyTokens);
        }
        if wire.variants.len() > MAX_VARIANTS {
            return Err(ThemeManifestError::TooManyVariants);
        }

        let mut tokens = BTreeMap::new();
        for (id, value) in wire.tokens {
            let token = ThemeToken::parse(&id)
                .ok_or_else(|| ThemeManifestError::UnknownToken(id.clone()))?;
            let value = token
                .parse_value(&value)
                .ok_or(ThemeManifestError::InvalidTokenValue(id))?;
            tokens.insert(token, value);
        }

        let mut window_chrome = WindowChromeVariant::Standard;
        for (id, value) in wire.variants {
            if id != "window.chrome" {
                return Err(ThemeManifestError::UnknownVariant(id));
            }
            window_chrome = WindowChromeVariant::parse(&value)
                .ok_or(ThemeManifestError::InvalidVariantValue(id))?;
        }

        let manifest = Self {
            id: wire.id,
            name: name.to_owned(),
            tokens,
            window_chrome,
        };
        ResolvedTheme::resolve(&manifest).validate_protected_contrast()?;
        Ok(manifest)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tokens(&self) -> &BTreeMap<ThemeToken, ThemeTokenValue> {
        &self.tokens
    }

    pub fn window_chrome(&self) -> WindowChromeVariant {
        self.window_chrome
    }
}

impl ResolvedTheme {
    pub fn stock() -> Self {
        Self {
            id: "com.rumahl.stock".to_owned(),
            name: "rumahl".to_owned(),
            tokens: stock_tokens(),
            window_chrome: WindowChromeVariant::Standard,
        }
    }

    pub fn resolve(manifest: &ThemeManifest) -> Self {
        let mut tokens = stock_tokens();
        tokens.extend(manifest.tokens.clone());
        Self {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            tokens,
            window_chrome: manifest.window_chrome,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tokens(&self) -> &BTreeMap<ThemeToken, ThemeTokenValue> {
        &self.tokens
    }

    pub fn window_chrome(&self) -> WindowChromeVariant {
        self.window_chrome
    }

    pub fn compile_css(&self) -> String {
        let mut css = String::from(":root {\n");
        for token in ThemeToken::ALL {
            let value = self
                .tokens
                .get(&token)
                .expect("resolved themes contain every public token");
            css.push_str("  ");
            css.push_str(token.css_property());
            css.push_str(": ");
            css.push_str(&value.as_css_value());
            css.push_str(";\n");
        }
        css.push_str("}\n");
        css
    }

    fn validate_protected_contrast(&self) -> Result<(), ThemeManifestError> {
        self.validate_contrast(
            ThemeToken::ColorTextPrimary,
            ThemeToken::ColorPanelBackground,
            "primary text",
        )?;
        self.validate_contrast(
            ThemeToken::ColorWindowTitlebarForeground,
            ThemeToken::ColorWindowTitlebarBackground,
            "window titlebar",
        )
    }

    fn validate_contrast(
        &self,
        foreground: ThemeToken,
        background: ThemeToken,
        protected_surface: &'static str,
    ) -> Result<(), ThemeManifestError> {
        let foreground = color_components(
            self.tokens
                .get(&foreground)
                .expect("resolved themes contain the protected foreground"),
        );
        let background = color_components(
            self.tokens
                .get(&background)
                .expect("resolved themes contain the protected background"),
        );
        let lighter = relative_luminance(foreground).max(relative_luminance(background));
        let darker = relative_luminance(foreground).min(relative_luminance(background));
        if (lighter + 0.05) / (darker + 0.05) < 4.5 {
            return Err(ThemeManifestError::InsufficientContrast(protected_surface));
        }
        Ok(())
    }
}

fn stock_tokens() -> BTreeMap<ThemeToken, ThemeTokenValue> {
    BTreeMap::from([
        (
            ThemeToken::ColorCanvasBackground,
            ThemeTokenValue::Color("#eef1f5".to_owned()),
        ),
        (
            ThemeToken::ColorPanelBackground,
            ThemeTokenValue::Color("#ffffff".to_owned()),
        ),
        (
            ThemeToken::ColorTextPrimary,
            ThemeTokenValue::Color("#15171a".to_owned()),
        ),
        (
            ThemeToken::ColorTextMuted,
            ThemeTokenValue::Color("#5c6470".to_owned()),
        ),
        (
            ThemeToken::ColorAccentPrimary,
            ThemeTokenValue::Color("#315efb".to_owned()),
        ),
        (
            ThemeToken::ColorWindowTitlebarBackground,
            ThemeTokenValue::Color("#ffffff".to_owned()),
        ),
        (
            ThemeToken::ColorWindowTitlebarForeground,
            ThemeTokenValue::Color("#15171a".to_owned()),
        ),
        (ThemeToken::RadiusWindow, ThemeTokenValue::Pixels(12)),
        (ThemeToken::SpaceShellGap, ThemeTokenValue::Pixels(16)),
    ])
}

fn parse_color(value: &str) -> Option<ThemeTokenValue> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(ThemeTokenValue::Color(format!("#{}", hex.to_lowercase())))
}

fn parse_pixels(value: &str, maximum: u8) -> Option<ThemeTokenValue> {
    let number = value.strip_suffix("px")?;
    if number.is_empty() || (number.len() > 1 && number.starts_with('0')) {
        return None;
    }
    let value = number.parse::<u8>().ok()?;
    (value <= maximum).then_some(ThemeTokenValue::Pixels(value))
}

fn color_components(value: &ThemeTokenValue) -> [u8; 3] {
    let ThemeTokenValue::Color(value) = value else {
        unreachable!("protected color tokens always contain colors")
    };
    let value = value
        .strip_prefix('#')
        .expect("validated colors always start with a hash");
    [
        u8::from_str_radix(&value[0..2], 16).expect("validated color component"),
        u8::from_str_radix(&value[2..4], 16).expect("validated color component"),
        u8::from_str_radix(&value[4..6], 16).expect("validated color component"),
    ]
}

fn relative_luminance(color: [u8; 3]) -> f64 {
    let linear = color.map(|component| {
        let component = f64::from(component) / 255.0;
        if component <= 0.04045 {
            component / 12.92
        } else {
            ((component + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

fn valid_namespaced_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_THEME_ID_BYTES || !value.is_ascii() {
        return false;
    }
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() >= 3
        && segments.into_iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_lowercase())
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !segment.ends_with('-')
        })
}

impl fmt::Display for ThemeManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "theme manifest exceeds the size limit"),
            Self::InvalidJson => write!(f, "theme manifest is invalid JSON"),
            Self::UnsupportedManifestVersion(_) => {
                write!(f, "theme manifest version is unsupported")
            }
            Self::UnsupportedUiContractVersion(_) => {
                write!(f, "theme UI contract version is unsupported")
            }
            Self::InvalidId => write!(f, "theme ID is invalid"),
            Self::InvalidName => write!(f, "theme name is invalid"),
            Self::TooManyTokens => write!(f, "theme declares too many tokens"),
            Self::TooManyVariants => write!(f, "theme declares too many variants"),
            Self::UnknownToken(_) => write!(f, "theme declares an unknown token"),
            Self::InvalidTokenValue(_) => write!(f, "theme token value is invalid"),
            Self::UnknownVariant(_) => write!(f, "theme declares an unknown variant"),
            Self::InvalidVariantValue(_) => write!(f, "theme variant value is invalid"),
            Self::InsufficientContrast(_) => {
                write!(f, "theme lowers contrast on a protected surface")
            }
        }
    }
}

impl Error for ThemeManifestError {}
