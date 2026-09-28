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
    // Legacy colour/geometry tokens (`--rumahl-*`).
    ColorCanvasBackground,
    ColorPanelBackground,
    ColorTextPrimary,
    ColorTextMuted,
    ColorAccentPrimary,
    ColorWindowTitlebarBackground,
    ColorWindowTitlebarForeground,
    RadiusWindow,
    SpaceShellGap,
    // Extended presentation tokens (`--rumahl-ui-*`).
    ColorAccent,
    ColorAccentStrong,
    ColorSurface,
    ColorSurfaceStrong,
    ColorOutline,
    ColorOnWallpaper,
    ColorOnAccent,
    ColorShadow,
    MaterialBlur,
    MaterialSaturation,
    ShapeRadiusDock,
    ShapeRadiusIcon,
    ShapeIconSize,
    ShapeIconSizeLarge,
    MotionDuration,
    MotionDurationFast,
    MotionEasingSpring,
    LayoutDockOffset,
    IconGradient,
    TextureWallpaper,
    TypographyFamily,
    TypographyScale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeTokenValue {
    Color(String),
    Pixels(u8),
    Millis(u16),
    Css(String),
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
    pub const ALL: [Self; 31] = [
        Self::ColorCanvasBackground,
        Self::ColorPanelBackground,
        Self::ColorTextPrimary,
        Self::ColorTextMuted,
        Self::ColorAccentPrimary,
        Self::ColorWindowTitlebarBackground,
        Self::ColorWindowTitlebarForeground,
        Self::RadiusWindow,
        Self::SpaceShellGap,
        Self::ColorAccent,
        Self::ColorAccentStrong,
        Self::ColorSurface,
        Self::ColorSurfaceStrong,
        Self::ColorOutline,
        Self::ColorOnWallpaper,
        Self::ColorOnAccent,
        Self::ColorShadow,
        Self::MaterialBlur,
        Self::MaterialSaturation,
        Self::ShapeRadiusDock,
        Self::ShapeRadiusIcon,
        Self::ShapeIconSize,
        Self::ShapeIconSizeLarge,
        Self::MotionDuration,
        Self::MotionDurationFast,
        Self::MotionEasingSpring,
        Self::LayoutDockOffset,
        Self::IconGradient,
        Self::TextureWallpaper,
        Self::TypographyFamily,
        Self::TypographyScale,
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
            Self::ColorAccent => "color.accent",
            Self::ColorAccentStrong => "color.accent.strong",
            Self::ColorSurface => "color.surface",
            Self::ColorSurfaceStrong => "color.surface.strong",
            Self::ColorOutline => "color.outline",
            Self::ColorOnWallpaper => "color.on.wallpaper",
            Self::ColorOnAccent => "color.on.accent",
            Self::ColorShadow => "color.shadow",
            Self::MaterialBlur => "material.blur",
            Self::MaterialSaturation => "material.saturation",
            Self::ShapeRadiusDock => "shape.radius.dock",
            Self::ShapeRadiusIcon => "shape.radius.icon",
            Self::ShapeIconSize => "shape.icon.size",
            Self::ShapeIconSizeLarge => "shape.icon.size.large",
            Self::MotionDuration => "motion.duration",
            Self::MotionDurationFast => "motion.duration.fast",
            Self::MotionEasingSpring => "motion.easing.spring",
            Self::LayoutDockOffset => "layout.dock.offset",
            Self::IconGradient => "icon.gradient",
            Self::TextureWallpaper => "texture.wallpaper",
            Self::TypographyFamily => "typography.family",
            Self::TypographyScale => "typography.scale",
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
            Self::ColorAccent => "--rumahl-ui-color-accent",
            Self::ColorAccentStrong => "--rumahl-ui-color-accent-strong",
            Self::ColorSurface => "--rumahl-ui-color-surface",
            Self::ColorSurfaceStrong => "--rumahl-ui-color-surface-strong",
            Self::ColorOutline => "--rumahl-ui-color-outline",
            Self::ColorOnWallpaper => "--rumahl-ui-color-on-wallpaper",
            Self::ColorOnAccent => "--rumahl-ui-color-on-accent",
            Self::ColorShadow => "--rumahl-ui-color-shadow",
            Self::MaterialBlur => "--rumahl-ui-material-blur",
            Self::MaterialSaturation => "--rumahl-ui-material-saturation",
            Self::ShapeRadiusDock => "--rumahl-ui-shape-radius-dock",
            Self::ShapeRadiusIcon => "--rumahl-ui-shape-radius-icon",
            Self::ShapeIconSize => "--rumahl-ui-shape-icon-size",
            Self::ShapeIconSizeLarge => "--rumahl-ui-shape-icon-size-large",
            Self::MotionDuration => "--rumahl-ui-motion-duration",
            Self::MotionDurationFast => "--rumahl-ui-motion-duration-fast",
            Self::MotionEasingSpring => "--rumahl-ui-motion-easing-spring",
            Self::LayoutDockOffset => "--rumahl-ui-layout-dock-offset",
            Self::IconGradient => "--rumahl-ui-icon-gradient",
            Self::TextureWallpaper => "--rumahl-ui-texture-wallpaper",
            Self::TypographyFamily => "--rumahl-ui-typography-family",
            Self::TypographyScale => "--rumahl-ui-typography-scale",
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
            Self::RadiusWindow => parse_pixels_between(value, 0, 32),
            Self::SpaceShellGap => parse_pixels_between(value, 0, 64),
            Self::MaterialBlur => parse_pixels_between(value, 0, 64),
            Self::ShapeRadiusDock => parse_pixels_between(value, 0, 64),
            Self::ShapeRadiusIcon => parse_pixels_between(value, 0, 64),
            Self::MaterialSaturation => parse_css_value(value),
            Self::ShapeIconSize => parse_pixels_between(value, 16, 128),
            Self::ShapeIconSizeLarge => parse_pixels_between(value, 16, 192),
            Self::MotionDuration | Self::MotionDurationFast => parse_millis(value, 0, 2_000),
            Self::LayoutDockOffset => parse_pixels_between(value, 0, 128),
            _ => parse_css_value(value),
        }
    }
}

impl ThemeTokenValue {
    pub fn as_css_value(&self) -> String {
        match self {
            Self::Color(value) | Self::Css(value) => value.clone(),
            Self::Pixels(value) => format!("{value}px"),
            Self::Millis(value) => format!("{value}ms"),
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
                .ok_or_else(|| ThemeManifestError::InvalidTokenValue(id.clone()))?;
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
            // The `default` sentinel means "the shell's bundled asset"; the
            // client substitutes a concrete URL, so it is not emitted here.
            if matches!(value, ThemeTokenValue::Css(text) if text == "default") {
                continue;
            }
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
    use ThemeToken::*;
    BTreeMap::from([
        (ColorCanvasBackground, color("#e8ece7")),
        (ColorPanelBackground, color("#f8faf7")),
        (ColorTextPrimary, color("#18201c")),
        (ColorTextMuted, color("#657068")),
        (ColorAccentPrimary, color("#28694c")),
        (ColorWindowTitlebarBackground, color("#f2f5f1")),
        (ColorWindowTitlebarForeground, color("#18201c")),
        (RadiusWindow, ThemeTokenValue::Pixels(12)),
        (SpaceShellGap, ThemeTokenValue::Pixels(16)),
        (ColorAccent, color("#28694c")),
        (ColorAccentStrong, color("#4f8f78")),
        (ColorSurface, color("#f8faf7d6")),
        (ColorSurfaceStrong, color("#f8faf7")),
        (ColorOutline, color("#ffffff70")),
        (ColorOnWallpaper, color("#ffffff")),
        (ColorOnAccent, color("#ffffff")),
        (ColorShadow, color("#00000045")),
        (MaterialBlur, css("34px")),
        (MaterialSaturation, css("1.6")),
        (ShapeRadiusDock, ThemeTokenValue::Pixels(22)),
        (ShapeRadiusIcon, ThemeTokenValue::Pixels(15)),
        (ShapeIconSize, ThemeTokenValue::Pixels(50)),
        (ShapeIconSizeLarge, ThemeTokenValue::Pixels(64)),
        (MotionDuration, ThemeTokenValue::Millis(150)),
        (MotionDurationFast, ThemeTokenValue::Millis(120)),
        (MotionEasingSpring, css("cubic-bezier(.2, .9, .3, 1.25)")),
        (LayoutDockOffset, ThemeTokenValue::Pixels(12)),
        (IconGradient, css("linear-gradient(140deg, #3f9e74, #28694c)")),
        (TextureWallpaper, css("default")),
        (TypographyFamily, css("-apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif")),
        (TypographyScale, css("1")),
    ])
}

fn color(value: &str) -> ThemeTokenValue {
    ThemeTokenValue::Color(value.to_owned())
}

fn css(value: &str) -> ThemeTokenValue {
    ThemeTokenValue::Css(value.to_owned())
}

fn parse_color(value: &str) -> Option<ThemeTokenValue> {
    let hex = value.strip_prefix('#')?;
    if (hex.len() == 6 || hex.len() == 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(ThemeTokenValue::Color(format!("#{}", hex.to_lowercase())))
    } else {
        None
    }
}

fn parse_pixels_between(value: &str, minimum: u8, maximum: u8) -> Option<ThemeTokenValue> {
    let number = value.strip_suffix("px")?;
    if number.is_empty() || (number.len() > 1 && number.starts_with('0')) {
        return None;
    }
    let parsed = number.parse::<u8>().ok()?;
    (parsed >= minimum && parsed <= maximum).then_some(ThemeTokenValue::Pixels(parsed))
}

fn parse_millis(value: &str, minimum: u16, maximum: u16) -> Option<ThemeTokenValue> {
    let number = value.strip_suffix("ms")?;
    if number.is_empty() || (number.len() > 1 && number.starts_with('0')) {
        return None;
    }
    let parsed = number.parse::<u16>().ok()?;
    (parsed >= minimum && parsed <= maximum).then_some(ThemeTokenValue::Millis(parsed))
}

/// Strict, non-executable CSS value: rejects declaration/rule breakouts and
/// only permits same-origin shell assets for `url(...)`.
fn parse_css_value(value: &str) -> Option<ThemeTokenValue> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 {
        return None;
    }
    let safe = value
        .chars()
        .all(|c| !c.is_control() && !matches!(c, ';' | '{' | '}' | '\\' | '<' | '>' | '@'));
    if !safe {
        return None;
    }
    if value.contains("url(") {
        let inner = value.strip_prefix("url(")?.strip_suffix(')')?;
        let path = inner.trim_matches(['"', '\'']);
        if !path.starts_with("/shell/assets/") || path.contains("..") {
            return None;
        }
    }
    Some(ThemeTokenValue::Css(value.to_owned()))
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
