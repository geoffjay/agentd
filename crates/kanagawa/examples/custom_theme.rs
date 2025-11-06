//! Example demonstrating how to make a custom theme.
//! Two options are provided; setting colors one-by-one, or using a helper macro.
use kanagawa::{Color, Theme, ThemeColors};

fn americano_simple() -> Theme {
    let mut oled = kanagawa::PALETTE.dragon;

    oled.colors.base.hex = (0, 0, 0).into();
    oled.colors.base.rgb = (0, 0, 0).into();
    oled.colors.base.hsl = (0.0, 0.0, 0.0).into();

    oled.colors.mantle.hex = (10, 10, 10).into();
    oled.colors.mantle.rgb = (10, 10, 10).into();
    oled.colors.mantle.hsl = (0.0, 0.0, 0.04).into();

    oled.colors.crust.hex = (0, 0, 0).into();
    oled.colors.crust.rgb = (0, 0, 0).into();
    oled.colors.crust.hsl = (0.0, 0.0, 0.08).into();

    oled
}

macro_rules! custom_theme {
    ($base:expr, $($color_key:ident: $rgb:expr, $hsl:expr,)*) => {
        Theme {
            colors: ThemeColors {
                $($color_key: Color {
                    hex: $rgb.into(),
                    rgb: $rgb.into(),
                    hsl: $hsl.into(),
                    ..$base.colors.$color_key
                },)*
                ..$base.colors
            },
            ..$base
        }
    };
}

fn use_theme(theme: &Theme) {
    println!("bg: {}", theme.colors.base.hex);
    println!("bg2: {}", theme.colors.mantle.hex);
    println!("fg: {}", theme.colors.text.hex);
    println!("accent: {}", theme.colors.mauve.hex);
}

fn main() {
    println!("The simple way:");
    let theme = americano_simple();
    use_theme(&theme);
    println!();

    println!("Or with a macro:");
    let theme = custom_theme!(kanagawa::PALETTE.dragon,
        base: (0, 0, 0), (0.0, 0.0, 0.0),
        mantle: (10, 10, 10), (0.0, 0.0, 0.04),
        crust: (20, 20, 20), (0.0, 0.0, 0.08),
    );
    use_theme(&theme);
}
