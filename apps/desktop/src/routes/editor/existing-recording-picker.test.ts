import { describe, expect, it } from "vitest";

import { getExistingRecordingPickerOptions } from "./existing-recording-picker";

describe("existing recording picker", () => {
	it("selects Scoopy project directories on Windows", () => {
		expect(
			getExistingRecordingPickerOptions("windows", "C:\\Scoopy\\recordings"),
		).toEqual({
			defaultPath: "C:\\Scoopy\\recordings",
			directory: true,
			multiple: false,
		});
	});

	it.each(["macos", "linux"] as const)(
		"preserves the filtered file picker on %s",
		(platform) => {
			expect(
				getExistingRecordingPickerOptions(platform, "/Scoopy/recordings"),
			).toEqual({
				defaultPath: "/Scoopy/recordings",
				filters: [{ name: "Scoopy Recording", extensions: ["cap"] }],
				multiple: false,
			});
		},
	);
});
