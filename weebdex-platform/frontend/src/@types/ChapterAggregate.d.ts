declare global {
	interface ChapterAggregate {
		volume?: string;
		chapter?: string;
		entries: Record<string, ChapterAggregateEntry>;
	}

	interface ChapterAggregateEntry {
		published_at?: string;
		language: number;
		groups: number[];
	}

	interface ChapterAggregateResponse {
		chapters: ChapterAggregate[];
		groups: Group[];
		languages: string[];
	}
}
export {};
