use std::collections::HashMap;
use std::fmt;

/// A 2D point with x and y coordinates.
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Creates a new Point.
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Returns the origin point (0, 0).
    pub fn origin() -> Self {
        Point { x: 0.0, y: 0.0 }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// A shape with a name and color.
pub trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn name(&self) -> &str;
}

/// Named colors.
pub enum Color {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}

impl Color {
    pub fn as_rgb(&self) -> (u8, u8, u8) {
        match self {
            Color::Red => (255, 0, 0),
            Color::Green => (0, 255, 0),
            Color::Blue => (0, 0, 255),
            Color::Custom(r, g, b) => (*r, *g, *b),
        }
    }
}

pub mod geometry {
    use std::f64::consts::PI;

    pub fn circle_area(radius: f64) -> f64 {
        PI * radius * radius
    }

    pub fn rectangle_area(width: f64, height: f64) -> f64 {
        width * height
    }
}

/// Counts word frequencies in a string.
pub fn word_count(text: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}
