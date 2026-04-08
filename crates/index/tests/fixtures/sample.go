package geometry

import (
	"fmt"
	"math"
)

// Shape is the common interface for geometric shapes.
type Shape interface {
	Area() float64
	Perimeter() float64
	Name() string
}

// Point represents a 2D coordinate.
type Point struct {
	X float64
	Y float64
}

// Circle is a round shape defined by a center and radius.
type Circle struct {
	Center Point
	Radius float64
}

// Rectangle is an axis-aligned rectangle.
type Rectangle struct {
	TopLeft     Point
	BottomRight Point
}

// Area returns the area of the circle.
func (c Circle) Area() float64 {
	return math.Pi * c.Radius * c.Radius
}

// Perimeter returns the circumference of the circle.
func (c Circle) Perimeter() float64 {
	return 2 * math.Pi * c.Radius
}

// Name returns the shape name.
func (c Circle) Name() string {
	return "circle"
}

// Area returns the area of the rectangle.
func (r Rectangle) Area() float64 {
	width := math.Abs(r.BottomRight.X - r.TopLeft.X)
	height := math.Abs(r.BottomRight.Y - r.TopLeft.Y)
	return width * height
}

// Perimeter returns the perimeter of the rectangle.
func (r Rectangle) Perimeter() float64 {
	width := math.Abs(r.BottomRight.X - r.TopLeft.X)
	height := math.Abs(r.BottomRight.Y - r.TopLeft.Y)
	return 2 * (width + height)
}

// Name returns the shape name.
func (r Rectangle) Name() string {
	return "rectangle"
}

// NewCircle creates a new circle with the given radius centered at the origin.
func NewCircle(radius float64) Circle {
	return Circle{Center: Point{}, Radius: radius}
}

// NewRectangle creates a new rectangle from corner coordinates.
func NewRectangle(x1, y1, x2, y2 float64) Rectangle {
	return Rectangle{
		TopLeft:     Point{X: x1, Y: y1},
		BottomRight: Point{X: x2, Y: y2},
	}
}

// describe returns a human-readable description of a shape.
func describe(s Shape) string {
	return fmt.Sprintf("%s: area=%.2f perimeter=%.2f", s.Name(), s.Area(), s.Perimeter())
}
