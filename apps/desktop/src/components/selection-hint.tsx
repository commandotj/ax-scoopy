import { type as ostype } from "@tauri-apps/plugin-os";
import { Match, Show, Switch } from "solid-js";

export type SelectionHintProps = {
	show: boolean;
	message?: string;
	class?: string;
};

export default function SelectionHint(props: SelectionHintProps) {
	const os = ostype();

	return (
		<Show when={props.show}>
			<div
				class={`pointer-events-none absolute inset-0 z-40 flex items-center justify-center px-4 ${
					props.class ?? ""
				}`}
			>
				<div class="flex flex-col items-center gap-5 text-center text-white drop-shadow-md">
					<div
						class="scoopy-selection-hint-monitor mb-6 relative"
						aria-hidden="true"
					>
						<IconScoopyMonitor class="w-full h-full" />
						<div class="scoopy-selection-hint-screen-area">
							<div class="scoopy-selection-hint-selection" aria-hidden="true" />
							<div class="scoopy-selection-hint-cursor" aria-hidden="true">
								<Switch>
									<Match when={os === "macos"}>
										<IconScoopyCursorMacos class="w-full h-full" />
									</Match>
									<Match when={os === "windows"}>
										<IconScoopyCursorWindows class="w-full h-full" />
									</Match>
								</Switch>
							</div>
						</div>
					</div>
					<p class="text-base font-medium max-w-xs">
						{props.message ?? "Click and drag to select an area"}
					</p>
				</div>
			</div>
		</Show>
	);
}
