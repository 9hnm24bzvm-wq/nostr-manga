declare global {
	interface UserPreferences {
		items_per_page: string;
		languages: string[];
		show_safe: boolean;
		show_suggestive: boolean;
		show_erotic: boolean;
		show_hentai: boolean;
		tags: string[];
		translated_languages: string[];
		uploaders: User[];
		groups: ScanlationGroup[];

		version: number;
	}
}
export {};
