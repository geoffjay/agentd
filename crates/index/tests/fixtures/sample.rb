require "json"
require "net/http"
require_relative "lib/helpers"

module Geometry
  # Computes the distance between two points.
  def self.distance(x1, y1, x2, y2)
    Math.sqrt((x2 - x1)**2 + (y2 - y1)**2)
  end
end

class Animal
  attr_reader :name, :sound

  # Create a new Animal.
  #
  # @param name [String] the animal's name
  # @param sound [String] the sound it makes
  def initialize(name, sound)
    @name = name
    @sound = sound
  end

  # Return a greeting string.
  def speak
    "#{@name} says #{@sound}"
  end

  def to_s
    "Animal(#{@name})"
  end
end

class Dog < Animal
  # Create a new Dog.
  def initialize(name)
    super(name, "woof")
    @tricks = []
  end

  def fetch(item)
    "#{@name} fetches #{item}"
  end

  def learn_trick(trick)
    @tricks << trick
  end

  def self.species
    "Canis familiaris"
  end
end

module Serializable
  def to_json(*args)
    JSON.generate(as_json, *args)
  end

  def as_json
    instance_variables.each_with_object({}) do |var, hash|
      hash[var.to_s.delete("@")] = instance_variable_get(var)
    end
  end
end

def greet(name)
  "Hello, #{name}!"
end

def find_file(directory, filename)
  Dir.glob(File.join(directory, "**", filename)).first
end
