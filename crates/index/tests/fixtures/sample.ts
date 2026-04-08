import { EventEmitter } from 'events';
import type { ReadStream } from 'fs';

interface Repository<T> {
    findById(id: string): Promise<T | null>;
    save(entity: T): Promise<T>;
    delete(id: string): Promise<void>;
    findAll(): Promise<T[]>;
}

interface User {
    id: string;
    name: string;
    email: string;
}

class UserService {
    private repository: Repository<User>;

    constructor(repository: Repository<User>) {
        this.repository = repository;
    }

    async getUser(id: string): Promise<User | null> {
        return this.repository.findById(id);
    }

    async createUser(name: string, email: string): Promise<User> {
        const user: User = { id: crypto.randomUUID(), name, email };
        return this.repository.save(user);
    }

    async deleteUser(id: string): Promise<void> {
        return this.repository.delete(id);
    }
}

class Logger {
    private prefix: string;

    constructor(prefix: string) {
        this.prefix = prefix;
    }

    log(message: string): void {
        console.log(`[${this.prefix}] ${message}`);
    }

    error(message: string): void {
        console.error(`[${this.prefix}] ERROR: ${message}`);
    }
}

function identity<T>(value: T): T {
    return value;
}

function merge<T extends object, U extends object>(target: T, source: U): T & U {
    return { ...target, ...source };
}

const formatDate = (date: Date): string => date.toISOString().split('T')[0];
