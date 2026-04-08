import Foundation
import UIKit

protocol Drawable {
    var color: String { get }
    func draw() -> String
    func area() -> Double
}

protocol Nameable {
    var name: String { get }
}

class Shape: Drawable, Nameable {
    var color: String
    var name: String

    init(color: String, name: String) {
        self.color = color
        self.name = name
    }

    func draw() -> String {
        return "Drawing \(name) in \(color)"
    }

    func area() -> Double {
        return 0.0
    }

    func describe() -> String {
        return "\(name): color=\(color), area=\(area())"
    }
}

struct Circle {
    let radius: Double
    let center: Point

    func circumference() -> Double {
        return 2 * Double.pi * radius
    }

    func area() -> Double {
        return Double.pi * radius * radius
    }
}

struct Point {
    var x: Double
    var y: Double

    func distanceTo(_ other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

enum Direction {
    case north
    case south
    case east
    case west

    func opposite() -> Direction {
        switch self {
        case .north: return .south
        case .south: return .north
        case .east: return .west
        case .west: return .east
        }
    }
}

extension Circle {
    func scaledBy(_ factor: Double) -> Circle {
        return Circle(radius: radius * factor, center: center)
    }

    func contains(_ point: Point) -> Bool {
        return center.distanceTo(point) <= radius
    }
}

public func makeShape(name: String, color: String) -> Shape {
    return Shape(color: color, name: name)
}

func lerp(a: Double, b: Double, t: Double) -> Double {
    return a + (b - a) * t
}
