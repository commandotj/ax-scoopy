import { describe, expect, it } from "vitest";
import {
	resolveServerRequestPath,
	shouldUseLocalServerSessionForUrl,
} from "./server-url-routing";

describe("server-url-routing", () => {
	it("keeps production Scoopy Cloud on the hybrid desktop session", () => {
		expect(
			shouldUseLocalServerSessionForUrl(
				"https://example.com",
				"https://example.com",
				false,
			),
		).toBe(false);
	});

	it("treats equivalent Scoopy Cloud origins as the same production auth path", () => {
		expect(
			shouldUseLocalServerSessionForUrl(
				"https://example.com/",
				"https://example.com",
				false,
			),
		).toBe(false);
	});

	it("uses the local callback session for custom production origins", () => {
		expect(
			shouldUseLocalServerSessionForUrl(
				"https://scoopy-web-production-7301.up.railway.app",
				"https://example.com",
				false,
			),
		).toBe(true);
	});

	it("keeps development on the local callback session", () => {
		expect(
			shouldUseLocalServerSessionForUrl(
				"https://example.com",
				"https://example.com",
				true,
			),
		).toBe(true);
	});

	it("does not rewrite Scoopy Cloud API requests for the default origin", () => {
		const path = "https://example.com/api/desktop/user/profile";

		expect(
			resolveServerRequestPath(path, "https://example.com", "https://example.com"),
		).toBe(path);
	});

	it("does not rewrite Scoopy Cloud API requests for equivalent origins", () => {
		const path = "https://example.com/api/desktop/user/profile";

		expect(
			resolveServerRequestPath(path, "https://example.com/", "https://example.com"),
		).toBe(path);
	});

	it("rewrites packaged API requests to custom origins", () => {
		expect(
			resolveServerRequestPath(
				"https://example.com/api/desktop/user/profile?refresh=true#profile",
				"https://scoopy-web-production-7301.up.railway.app",
				"https://example.com",
			),
		).toBe(
			"https://scoopy-web-production-7301.up.railway.app/api/desktop/user/profile?refresh=true#profile",
		);
	});

	it("does not rewrite external API requests", () => {
		const path = "https://l.example.com/api/license/activate";

		expect(
			resolveServerRequestPath(
				path,
				"https://scoopy-web-production-7301.up.railway.app",
				"https://example.com",
			),
		).toBe(path);
	});
});
