/**
 * Detection of `${VAR}` / `${VAR:-default}` environment substitution
 * markers in template values.
 *
 * `agent apply` expands these at parse time (apply.rs
 * expand_env_in_value); the UI cannot, so imports keep the literal text
 * and surface a warning listing the variables found.
 */

const SUBSTITUTION_PATTERN = /\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^}]*)?\}/g;

/** Distinct `${VAR}` expressions found across the given values, in order. */
export function findEnvSubstitutions(values: string[]): string[] {
	const seen = new Set<string>();
	const found: string[] = [];
	for (const value of values) {
		for (const match of value.matchAll(SUBSTITUTION_PATTERN)) {
			if (!seen.has(match[0])) {
				seen.add(match[0]);
				found.push(match[0]);
			}
		}
	}
	return found;
}
