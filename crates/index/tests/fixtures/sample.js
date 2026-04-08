import { readFile } from 'fs/promises';
import path from 'path';

/**
 * A simple calculator with history.
 */
class Calculator {
    constructor() {
        this.history = [];
        this.result = 0;
    }

    add(a, b) {
        const result = a + b;
        this.history.push({ op: 'add', result });
        return result;
    }

    multiply(a, b) {
        return a * b;
    }

    clearHistory() {
        this.history = [];
    }
}

/**
 * An event emitter for simple pub/sub.
 */
class EventBus {
    constructor() {
        this.listeners = new Map();
    }

    on(event, handler) {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, []);
        }
        this.listeners.get(event).push(handler);
    }

    emit(event, data) {
        const handlers = this.listeners.get(event) || [];
        handlers.forEach(h => h(data));
    }
}

function greet(name) {
    return `Hello, ${name}!`;
}

function readConfig(filePath) {
    return readFile(path.resolve(filePath), 'utf8');
}

const square = (x) => x * x;
const clamp = (value, min, max) => Math.min(Math.max(value, min), max);
