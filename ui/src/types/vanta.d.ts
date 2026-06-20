// Vanta.js ships no type declarations. Declare the minimal surface we use:
// each effect factory takes an options object (including `el` and the `p5`
// constructor for p5-based effects like TOPOLOGY) and returns a handle whose
// `destroy()` tears the animation down.
declare module "vanta/dist/vanta.topology.min" {
	interface VantaEffect {
		destroy(): void;
	}
	const TOPOLOGY: (options: Record<string, unknown>) => VantaEffect;
	export default TOPOLOGY;
}
