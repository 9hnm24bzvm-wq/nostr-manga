import type { Constants } from "$lib";

declare global {
	interface Manga extends MangaAttributes {
		relationships: {
			available_languages?: string[];
			available_groups?: ScanlationGroup[];

			submitter?: User;
			cover?: Cover;
			tags?: Tag[];
			authors?: Tag[];
			artists?: Tag[];
			links?: Record<string, string>;
			relations?: Relation[];
			stats?: MangaStats;
			thread: Thread;

			_chapters?: Chapter[];
		};
		_is_preload?: boolean;
	}

	interface MangaAttributes {
		id: string;

		created_at: string;
		updated_at: string;
		published_at?: string;
		deleted_at?: string;

		state: Constants.State;
		title: string;
		alt_titles?: Record<string, string[]>;
		description?: string;
		year: number;
		language: string;
		demographic?: Constants.Manga.Demographic;
		status: Constants.Manga.PublicationStatus;
		content_rating: Constants.Manga.ContentRating;

		last_volume?: string;
		last_chapter?: string;

		locked?: boolean;
		version: number;
	}

	interface Relation extends MangaAttributes {
		type: Constants.Manga.RelationType;
		relationships: {
			cover?: Cover;
			tags?: Tag[];
		};
	}

	interface MangaStats {
		chapters: number;
		follows: number;
		views: number;
	}
}
export {};
