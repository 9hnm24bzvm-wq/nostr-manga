import type { Constants } from "$lib";

declare global {
	export interface MangaDraft {
		title: string;
		description?: string;
		year?: number;
		language: string;
		demographic?: Constants.Manga.Demographic;
		status: Constants.Manga.PublicationStatus;
		content_rating: Constants.Manga.ContentRating;

		last_volume?: string;
		last_chapter?: string;

		covers: CoverDraft[];
		alt_titles: AltTitle[];
		tags: string[];
		authors: Author[];
		artists: Author[];
		links: ExternalLink[];
		relations: RelationDraft[];
	}

	export interface CoverDraft extends Partial<Cover> {
		main?: boolean;
		file?: File;
		deleted?: boolean;
		valid: boolean;

		url: string;
		thumbnail: string;
	}

	export interface RelationDraft extends Relation {
		new?: boolean;
		deleted?: boolean;
	}

	export interface AltTitle {
		language: string;
		title: string;
		valid: boolean;
	}
}
export {};
