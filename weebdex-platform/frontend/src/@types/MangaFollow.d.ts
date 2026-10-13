import type { Constants } from "$lib";

declare global {
	interface MangaFollow extends Manga {
		reading_status: Constants.Manga.ReadingStatus;
		followed_at: string;
	}
}
export {};
