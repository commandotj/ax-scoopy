import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const srcDir = join(dirname(fileURLToPath(import.meta.url)), "../src");

// recorder-core ships as browser TypeScript sources, so every dependency has to
// stay type-only or bundler-safe. This test is what actually enforces that.
describe("package boundaries", () => {
	it("has no runtime dependencies", () => {
		const packageJson = JSON.parse(
			readFileSync(join(srcDir, "../package.json"), "utf8"),
		) as { dependencies?: Record<string, string> };
		expect(packageJson.dependencies ?? {}).toEqual({});
	});
});
