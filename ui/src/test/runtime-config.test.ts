import { HttpResponse, http } from "msw";
import { afterEach, describe, expect, it } from "vitest";
import {
	loadRuntimeConfig,
	runtimeServiceUrl,
	setRuntimeConfig,
} from "../runtime-config";
import { server } from "./mocks/server";

const CONFIG = {
	version: "0.4.3",
	services: {
		ask: { port: 7001 },
		orchestrator: { port: 7006, url: "https://agentd.example.com/orch" },
	},
};

afterEach(() => setRuntimeConfig(null));

describe("loadRuntimeConfig", () => {
	it("returns the parsed config when /config.json serves JSON", async () => {
		server.use(http.get("*/config.json", () => HttpResponse.json(CONFIG)));
		expect(await loadRuntimeConfig()).toEqual(CONFIG);
	});

	it("returns null when the endpoint is missing", async () => {
		server.use(
			http.get("*/config.json", () => new HttpResponse(null, { status: 404 })),
		);
		expect(await loadRuntimeConfig()).toBeNull();
	});

	it("returns null when an SPA fallback serves HTML", async () => {
		server.use(
			http.get(
				"*/config.json",
				() =>
					new HttpResponse("<!doctype html><html></html>", {
						headers: { "content-type": "text/html" },
					}),
			),
		);
		expect(await loadRuntimeConfig()).toBeNull();
	});

	it("returns null when the payload fails validation", async () => {
		server.use(
			http.get("*/config.json", () =>
				HttpResponse.json({ services: { ask: { port: "7001" } } }),
			),
		);
		expect(await loadRuntimeConfig()).toBeNull();
	});
});

describe("runtimeServiceUrl", () => {
	it("returns undefined when no runtime config is loaded", () => {
		expect(runtimeServiceUrl("ask")).toBeUndefined();
	});

	it("derives the URL from the page protocol and hostname plus the port", () => {
		setRuntimeConfig(CONFIG);
		const { protocol, hostname } = window.location;
		expect(runtimeServiceUrl("ask")).toBe(`${protocol}//${hostname}:7001`);
	});

	it("prefers an explicit url override", () => {
		setRuntimeConfig(CONFIG);
		expect(runtimeServiceUrl("orchestrator")).toBe(
			"https://agentd.example.com/orch",
		);
	});

	it("returns undefined for an unknown service", () => {
		setRuntimeConfig(CONFIG);
		expect(runtimeServiceUrl("hook")).toBeUndefined();
	});
});
