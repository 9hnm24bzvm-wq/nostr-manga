import type { Constants } from "$lib";
import type { Writable } from "svelte/store";

declare global {
	interface ReaderPreferences {
		scale: Constants.Reader.Scale;
		layout: Constants.Reader.Layout;
		direction: Constants.Reader.Direction;

		double: boolean;
		offset: number;

		width: number;
		gap: number;
		scroll: number;

		preload: number;
		parallel: number;

		tap_navigation: boolean;
		keyboard_navigation: boolean;
		scroll_navigation: boolean;
		auto_advance: boolean;

		panel: boolean;
		panel_position: "left" | "right";

		version: number;
	}

	interface ReaderPage {
		data: Page;
		index: number;

		blob?: string;
		ref: Writable<HTMLElement | undefined>;

		isLoaded?: boolean;
		isLoading?: boolean;
		isFailed?: boolean;
	}

	interface ReaderScrollState {
		locked: boolean;
		scrolling: boolean;
		direction?: "top" | "bottom";
	}

	interface ReaderChapterInfo {
		id: string;
		volume?: string;
		chapter?: string;
		language: string;
		groups: string[];
	}
}
export {};
