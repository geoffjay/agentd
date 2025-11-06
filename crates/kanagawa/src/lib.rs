//! 🦀 Kanagawa theme for Rust.
//!
//! # Usage
//!
//! Add Kanagawa to your project's `Cargo.toml`:
//!
//! ```console
//! $ cargo add kanagawa
//! ```
//!
//! # Example
//!
//! ```rust
//! struct Button {
//!     text: String,
//!     background_color: String,
//! };
//!
//! fn confirm(text: String) -> Button {
//!     Button {
//!         text,
//!         background_color: kanagawa::PALETTE.dragon.colors.green.hex.to_string(),
//!     }
//! }
//! ```
//!
//! More examples can be found
//! [here](https://github.com/geoffjay/kanagawa/tree/main/examples).
//!
//! # Optional Features
//!
//! ## ANSI string painting
//!
//! Enable the `ansi-term` feature to add the
//! [`Color::ansi_paint`](Color::ansi_paint) method.
//! This adds [ansi-term](https://crates.io/crates/ansi_term) as a dependency.
//!
//! Example: [`examples/term_grid.rs`](https://github.com/geoffjay/kanagawa/blob/main/examples/term_grid.rs)
//!
//! ### CSS colors
//!
//! Enable the `css-colors` feature to enable the conversion of Kanagawa colors to
//! [`css_colors::RGB`] instances.
//! This adds [css-colors](https://crates.io/crates/css-colors) as a dependency.
//!
//! Example: [`examples/css.rs`](https://github.com/geoffjay/kanagawa/blob/main/examples/css.rs)
//!
//! ### Ratatui
//!
//! Enable the `ratatui` feature to enable the conversion of Kanagawa colors to
//! [`ratatui::style::Color`] instances.
//! This adds [ratatui](https://crates.io/crates/ratatui) as a dependency.
//!
//! Example: [`examples/ratatui.rs`](https://github.com/geoffjay/kanagawa/blob/main/examples/ratatui.rs)
//!
//! ### Serde
//!
//! Enable the `serde` feature to enable the serialization of Kanagawa's palette,
//! theme, and color types.
//! This adds [serde](https://crates.io/crates/serde) as a dependency.
//!
//! Example: [`examples/serde.rs`](https://github.com/geoffjay/kanagawa/blob/main/examples/serde.rs)
use std::{fmt, marker::PhantomData, ops::Index, str::FromStr};

include!(concat!(env!("OUT_DIR"), "/generated_palette.rs"));

/// The top-level type that encompasses the Kanagawa palette data structure.
/// Primarily used via the [`PALETTE`] constant.
///
/// Can be iterated over, in which case the themes are yielded in the canonical order:
/// Lotus, Wave, Dragon.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Palette {
    /// The light theme.
    pub lotus: Theme,
    /// The medium dark theme.
    pub wave: Theme,
    /// The dark dark theme.
    pub dragon: Theme,
}

/// Enum of all four themes of Kanagawa. Can be used to index [`Palette`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ThemeName {
    /// The light theme.
    Lotus,
    /// The medium dark theme.
    Wave,
    /// The dark dark theme.
    Dragon,
}

/// An iterator over themes in the palette.
/// Obtained via [`Palette::iter()`].
pub struct ThemeIterator<'a> {
    current: usize,
    phantom: PhantomData<&'a ()>,
}

/// Color represented as individual red, green, and blue channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// Color represented as 6-digit hexadecimal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hex(Rgb);

/// Color represented as individual hue (0-359), saturation (0-1), and lightness (0-1) channels.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hsl {
    /// Hue channel.
    pub h: f64,
    /// Saturation channel.
    pub s: f64,
    /// Lightness channel.
    pub l: f64,
}

/// A single color in the Kanagawa palette.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color {
    /// The [`ColorName`] for this color.
    pub name: ColorName,
    /// Order of the color in the palette spec.
    pub order: u32,
    /// Whether the color is considered an accent color.
    /// Accent colors are the first 14 colors in the palette, also called
    /// the analogous colours. The remaining 12 non-accent colors are also
    /// referred to as the monochromatic colors.
    pub accent: bool,
    /// The color represented as a six-digit hex string with a leading hash (#).
    pub hex: Hex,
    /// The color represented as individual red, green, and blue channels.
    pub rgb: Rgb,
    /// The color represented as individual hue, saturation, and lightness channels.
    pub hsl: Hsl,
}

/// A theme is a collection of colors. Kanagawa has three themes; Lotus,
/// Wave, and Dragon.
///
/// Can be iterated over, in which case the colors are yielded in order.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Theme {
    /// The name of the theme.
    pub name: ThemeName,
    /// Emoji associated with the theme. Requires Unicode 13.0 (2020) or later to render.
    pub emoji: char,
    /// Order of the theme in the palette spec.
    pub order: u32,
    /// Whether this theme is dark or light oriented. Lotus is light, the other
    /// three themes are dark.
    pub dark: bool,
    /// The colors in the theme.
    pub colors: ThemeColors,
    /// The ANSI colors in the theme.
    pub ansi_colors: ThemeAnsiColors,
}

/// An iterator over colors in a theme.
/// Obtained via [`Theme::into_iter()`](struct.Theme.html#method.into_iter) or [`ThemeColors::iter()`].
pub struct ColorIterator<'a> {
    colors: &'a ThemeColors,
    current: usize,
}

/// An iterator over the ANSI colors in a theme.
///
/// Defaults to ascending order by ANSI code 0 -> 16.
/// Obtained via [`ThemeAnsiColors::into_iter()`](struct.ThemeAnsiColors.html#method.into_iter) or [`ThemeAnsiColors::iter()`].
pub struct AnsiColorIterator<'a> {
    ansi_colors: &'a ThemeAnsiColors,
    current: usize,
}

/// An iterator over the ANSI color pairs in a theme.
/// Obtained via [`ThemeAnsiColorPairs::into_iter()`](struct.ThemeAnsiColorPairs.html#method.into_iter) or [`ThemeAnsiColorPairs::iter()`].
pub struct AnsiColorPairsIterator<'a> {
    ansi_color_pairs: &'a ThemeAnsiColorPairs,
    current: usize,
}

impl Palette {
    /// Get an array of the themes in the palette.
    #[must_use]
    pub const fn all_themes(&self) -> [&Theme; 3] {
        [&self.lotus, &self.wave, &self.dragon]
    }

    /// Create an iterator over the themes in the palette.
    #[must_use]
    pub const fn iter(&self) -> ThemeIterator<'_> {
        ThemeIterator { current: 0, phantom: PhantomData }
    }
}

impl Index<ThemeName> for Palette {
    type Output = Theme;

    fn index(&self, index: ThemeName) -> &Self::Output {
        match index {
            ThemeName::Lotus => &self.lotus,
            ThemeName::Wave => &self.wave,
            ThemeName::Dragon => &self.dragon,
        }
    }
}

impl Palette {
    /// Get a theme by name.
    ///
    /// This is equivalent to using the index operator, but can also be used in
    /// const contexts.
    #[must_use]
    pub const fn get_theme(&self, name: ThemeName) -> &Theme {
        match name {
            ThemeName::Lotus => &self.lotus,
            ThemeName::Wave => &self.wave,
            ThemeName::Dragon => &self.dragon,
        }
    }
}

impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Rgb { r, g, b } = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}")
    }
}

#[cfg(feature = "serde")]
mod _hex {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::{Hex, Rgb};

    impl Serialize for Hex {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.to_string())
        }
    }

    impl<'de> Deserialize<'de> for Hex {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let hex: String = Deserialize::deserialize(deserializer)?;
            let hex: u32 = u32::from_str_radix(hex.trim_start_matches('#'), 16)
                .map_err(serde::de::Error::custom)?;
            let r = ((hex >> 16) & 0xff) as u8;
            let g = ((hex >> 8) & 0xff) as u8;
            let b = (hex & 0xff) as u8;
            Ok(Self(Rgb { r, g, b }))
        }
    }
}

impl fmt::Display for ThemeName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Lotus => write!(f, "Lotus"),
            Self::Wave => write!(f, "Wave"),
            Self::Dragon => write!(f, "Dragon"),
        }
    }
}

/// Error type for parsing a [`ThemeName`] from a string.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseThemeNameError;
impl std::error::Error for ParseThemeNameError {}
impl std::fmt::Display for ParseThemeNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid theme identifier, expected one of: lotus, wave, dragon")
    }
}

impl FromStr for ThemeName {
    type Err = ParseThemeNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lotus" => Ok(Self::Lotus),
            "wave" => Ok(Self::Wave),
            "dragon" => Ok(Self::Dragon),
            _ => Err(ParseThemeNameError),
        }
    }
}

impl ThemeName {
    /// Get the theme's identifier; the lowercase key used to identify the theme.
    /// This differs from `to_string` in that it's intended for machine usage
    /// rather than presentation.
    ///
    /// Example:
    ///
    /// ```rust
    /// let lotus = kanagawa::PALETTE.lotus;
    /// assert_eq!(lotus.name.identifier(), "lotus");
    /// ```
    #[must_use]
    pub const fn identifier(&self) -> &'static str {
        match self {
            Self::Lotus => "lotus",
            Self::Wave => "wave",
            Self::Dragon => "dragon",
        }
    }
}

impl ThemeColors {
    /// Create an iterator over the colors in the theme.
    #[must_use]
    pub const fn iter(&self) -> ColorIterator<'_> {
        ColorIterator { colors: self, current: 0 }
    }
}

impl ThemeAnsiColors {
    /// Create an iterator over the ANSI colors in the theme.
    #[must_use]
    pub const fn iter(&self) -> AnsiColorIterator<'_> {
        AnsiColorIterator { ansi_colors: self, current: 0 }
    }

    /// Get the ANSI color pairs
    #[must_use]
    pub const fn all_pairs(&self) -> ThemeAnsiColorPairs {
        self.to_ansi_color_pairs()
    }
}

impl ThemeAnsiColorPairs {
    /// Create an iterator over the ANSI color pairs in the theme.
    #[must_use]
    pub const fn iter(&self) -> AnsiColorPairsIterator<'_> {
        AnsiColorPairsIterator { ansi_color_pairs: self, current: 0 }
    }
}

impl<'a> Iterator for ThemeIterator<'a> {
    type Item = &'a Theme;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= PALETTE.all_themes().len() {
            None
        } else {
            let theme = PALETTE.all_themes()[self.current];
            self.current += 1;
            Some(theme)
        }
    }
}

impl<'a> Iterator for ColorIterator<'a> {
    type Item = &'a Color;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.colors.all_colors().len() {
            None
        } else {
            let color = self.colors.all_colors()[self.current];
            self.current += 1;
            Some(color)
        }
    }
}

impl<'a> Iterator for AnsiColorIterator<'a> {
    type Item = &'a AnsiColor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.ansi_colors.all_ansi_colors().len() {
            None
        } else {
            let color = self.ansi_colors.all_ansi_colors()[self.current];
            self.current += 1;
            Some(color)
        }
    }
}

impl<'a> Iterator for AnsiColorPairsIterator<'a> {
    type Item = &'a AnsiColorPair;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.ansi_color_pairs.all_ansi_color_pairs().len() {
            None
        } else {
            let color = self.ansi_color_pairs.all_ansi_color_pairs()[self.current];
            self.current += 1;
            Some(color)
        }
    }
}

impl<'a> IntoIterator for &'a Palette {
    type Item = &'a Theme;
    type IntoIter = ThemeIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a ThemeColors {
    type Item = &'a Color;
    type IntoIter = ColorIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a ThemeAnsiColors {
    type Item = &'a AnsiColor;
    type IntoIter = AnsiColorIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a ThemeAnsiColorPairs {
    type Item = &'a AnsiColorPair;
    type IntoIter = AnsiColorPairsIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Theme {
    /// Create an iterator over the colors in the theme.
    #[must_use]
    pub const fn iter(&self) -> ColorIterator<'_> {
        self.colors.iter()
    }

    /// Equivalent to [`<theme>.name.identifier()`](ThemeName::identifier).
    #[must_use]
    pub const fn identifier(&self) -> &'static str {
        self.name.identifier()
    }
}

impl<'a> IntoIterator for &'a Theme {
    type Item = &'a Color;
    type IntoIter = ColorIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.colors.iter()
    }
}

/// Error type for parsing a [`ColorName`] from a string.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseColorNameError;
impl std::error::Error for ParseColorNameError {}
impl std::fmt::Display for ParseColorNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid color identifier")
    }
}

impl Index<ColorName> for Theme {
    type Output = Color;

    fn index(&self, index: ColorName) -> &Self::Output {
        self.colors.index(index)
    }
}

impl Theme {
    /// Get a color by name.
    ///
    /// This is equivalent to using the index operator, but can also be used in
    /// const contexts.
    #[must_use]
    pub const fn get_color(&self, name: ColorName) -> &Color {
        self.colors.get_color(name)
    }
}

impl Color {
    /// Equivalent to [`<color>.name.identifier()`](ColorName::identifier).
    #[must_use]
    pub const fn identifier(&self) -> &'static str {
        self.name.identifier()
    }
}

impl From<(u8, u8, u8)> for Rgb {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self { r, g, b }
    }
}

impl From<(u8, u8, u8)> for Hex {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self(Rgb { r, g, b })
    }
}

impl From<(f64, f64, f64)> for Hsl {
    fn from((h, s, l): (f64, f64, f64)) -> Self {
        Self { h, s, l }
    }
}

#[cfg(feature = "ansi-term")]
mod ansi_term {
    use crate::{AnsiColor, Color};

    impl Color {
        /// Paints the given input with a color à la [ansi_term](https://docs.rs/ansi_term/latest/ansi_term/)
        pub fn ansi_paint<'a, I, S: 'a + ToOwned + ?Sized>(
            &self,
            input: I,
        ) -> ansi_term::ANSIGenericString<'a, S>
        where
            I: Into<std::borrow::Cow<'a, S>>,
            <S as ToOwned>::Owned: core::fmt::Debug,
        {
            ansi_term::Color::RGB(self.rgb.r, self.rgb.g, self.rgb.b).paint(input)
        }
    }

    impl AnsiColor {
        /// Paints the given input with a color à la [ansi_term](https://docs.rs/ansi_term/latest/ansi_term/)
        pub fn ansi_paint<'a, I, S: 'a + ToOwned + ?Sized>(
            &self,
            input: I,
        ) -> ansi_term::ANSIGenericString<'a, S>
        where
            I: Into<std::borrow::Cow<'a, S>>,
            <S as ToOwned>::Owned: core::fmt::Debug,
        {
            ansi_term::Color::RGB(self.rgb.r, self.rgb.g, self.rgb.b).paint(input)
        }
    }
}

#[cfg(feature = "css-colors")]
mod css_colors {
    use crate::{AnsiColor, Color};

    impl From<Color> for css_colors::RGB {
        fn from(value: Color) -> Self {
            Self {
                r: css_colors::Ratio::from_u8(value.rgb.r),
                g: css_colors::Ratio::from_u8(value.rgb.g),
                b: css_colors::Ratio::from_u8(value.rgb.b),
            }
        }
    }

    impl From<AnsiColor> for css_colors::RGB {
        fn from(value: AnsiColor) -> Self {
            Self {
                r: css_colors::Ratio::from_u8(value.rgb.r),
                g: css_colors::Ratio::from_u8(value.rgb.g),
                b: css_colors::Ratio::from_u8(value.rgb.b),
            }
        }
    }

    impl From<Color> for css_colors::HSL {
        fn from(value: Color) -> Self {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Self {
                h: css_colors::Angle::new(value.hsl.h as u16),
                s: css_colors::Ratio::from_f32(value.hsl.s as f32),
                l: css_colors::Ratio::from_f32(value.hsl.l as f32),
            }
        }
    }

    impl From<AnsiColor> for css_colors::HSL {
        fn from(value: AnsiColor) -> Self {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Self {
                h: css_colors::Angle::new(value.hsl.h as u16),
                s: css_colors::Ratio::from_f32(value.hsl.s as f32),
                l: css_colors::Ratio::from_f32(value.hsl.l as f32),
            }
        }
    }
}

#[cfg(feature = "ratatui")]
mod ratatui {
    use crate::{AnsiColor, Color};

    impl From<Color> for ratatui::style::Color {
        fn from(value: Color) -> Self {
            Self::Rgb(value.rgb.r, value.rgb.g, value.rgb.b)
        }
    }

    impl From<AnsiColor> for ratatui::style::Color {
        fn from(value: AnsiColor) -> Self {
            Self::Rgb(value.rgb.r, value.rgb.g, value.rgb.b)
        }
    }
}
