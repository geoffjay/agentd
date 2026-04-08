import os
import sys
from pathlib import Path
from typing import Optional, List


class Animal:
    """Base class for all animals."""

    def __init__(self, name: str, sound: str) -> None:
        self.name = name
        self.sound = sound

    def speak(self) -> str:
        """Return the animal's greeting."""
        return f"{self.name} says {self.sound}"

    def __repr__(self) -> str:
        return f"Animal({self.name!r})"


class Dog(Animal):
    """A dog that can fetch."""

    def __init__(self, name: str) -> None:
        super().__init__(name, "woof")
        self._tricks: List[str] = []

    def fetch(self, item: str) -> str:
        return f"{self.name} fetches {item}"

    def learn_trick(self, trick: str) -> None:
        self._tricks.append(trick)


def greet(name: str) -> str:
    """Return a greeting message."""
    return f"Hello, {name}!"


def find_file(directory: str, filename: str) -> Optional[Path]:
    """Search for a file in a directory tree."""
    root = Path(directory)
    for path in root.rglob(filename):
        return path
    return None


async def fetch_data(url: str) -> bytes:
    """Async placeholder for fetching remote data."""
    raise NotImplementedError
