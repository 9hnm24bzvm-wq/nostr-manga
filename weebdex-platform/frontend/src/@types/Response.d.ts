declare global {
	interface CheckSessionResponse {
		identity: {
			id: string;
			internal_id: string;
			email: string;
			username: string;
			is_verified: boolean;
		};

		preferences: UserPreferences;

		follows: Record<string, [ReadingStatus, 0 | 1]>;
		groups?: ScanlationGroup[];
		roles?: string[];
		permissions?: string[];
	}

	interface FeedResponse<T> {
		total: number;
		limit: number;
		page: number;
		data?: T[];
	}

	type UserFollowFeedResponse = FeedResponse<UserFollow>;
	type MangaFollowFeedResponse = FeedResponse<MangaFollow>;
	type GroupFollowFeedResponse = FeedResponse<GroupFollow>;

	type MangaFeedResponse = FeedResponse<Manga>;
	type ChapterFeedResponse = FeedResponse<Chapter>;
	type UserFeedResponse = FeedResponse<User>;
	type TagFeedResponse = FeedResponse<Tag>;
	type ScanlationGroupFeedResponse = FeedResponse<ScanlationGroup>;
	type ReportFeedResponse = FeedResponse<Report>;
	type ReplyFeedResponse = FeedResponse<Reply>;

	interface MappedFeedResponse<T, U> {
		total: number;
		limit: number;
		page: number;
		map: U;
		data?: T[];
	}

	type MappedChapterFeedResponse = MappedFeedResponse<Chapter, { manga: Record<string, Manga> }>;
	type MappedChapterHistoryFeedResponse = MappedFeedResponse<ChapterHistory, { manga: Record<string, Manga> }>;

	interface UpdateChaptersHistoryBatchResponse {
		read_updated: number;
		unread_updated: number;
	}
}
export {};
