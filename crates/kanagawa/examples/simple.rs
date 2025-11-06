//! Simple example showing how to get colors from the Kanagawa palette.
use kanagawa::{AnsiColor, ColorName, Rgb, PALETTE};

fn main() {
    let lotus_teal = PALETTE.lotus.colors.teal;
    let Rgb { r, g, b } = lotus_teal.rgb;
    println!("Lotus's {} is {}, which is rgb({r}, {g}, {b})", lotus_teal.name, lotus_teal.hex);

    // you can also get a color by its name, from `ThemeColors` or `Theme`:
    let dragon = &PALETTE.dragon;
    let dragon_teal = dragon.colors[ColorName::Teal];
    let dragon_mauve = dragon[ColorName::Mauve];

    let Rgb { r, g, b } = dragon_teal.rgb;
    println!("Dragon's {} is {}, which is rgb({r}, {g}, {b})", dragon_teal.name, dragon_teal.hex);

    println!("Dragon's {} is {}", dragon_mauve.name, dragon_mauve.hex);
    println!();

    // iterate over the 16 ANSI colors (i.e. Black, Red, ..., Bright Black, Bright Red, ...)
    println!("Dragon's ANSI colors in code order:");
    for AnsiColor { name, rgb, hsl, code, hex } in &dragon.ansi_colors {
        println!(
            "Dragon ANSI [{:2}] {:15} →  {:6}  {:3?}  {:19?}",
            code,
            name.to_string(),
            hex,
            rgb,
            hsl,
        );
    }
    println!();

    // iterate over the 16 ANSI colors in 8 pairs (i.e. Black, Bright Black, Red, Bright Red, ...)
    println!("Dragon's ANSI color pairs:");
    for pair in &dragon.ansi_colors.all_pairs() {
        println!(
            "[{:2}] {:7} / [{:2}] {}",
            pair.normal.code,
            pair.normal.name.to_string(),
            pair.bright.code,
            pair.bright.name
        );
    }
}
