declare global {
	interface Page {
		name: string;
		dimensions: number[];
	}

	interface Chapter {
		id: string;
		source_id?: string;

		created_at: string;
		updated_at: string;
		published_at?: string;
		deleted_at?: string;
		read_at?: string;

		title?: string;
		volume?: string;
		chapter?: string;
		language: string;
		is_unavailable?: boolean;

		node?: string;
		data?: Page[];
		data_optimized?: Page[];
		version: number;

		relationships: {
			uploader?: User;
			groups?: ScanlationGroup[];
			manga: Manga;
			stats?: ChapterStats;
			thread: Thread;
		};
	}

	interface ChapterStats {
		views: number;
	}
}
export {};
