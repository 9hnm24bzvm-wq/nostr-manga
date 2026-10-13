import type { Constants } from "$lib";

declare global {
	interface Tag {
		id: string;
		group: Constants.Tag.Group;
		name: string;
	}
}
export {};
