export enum SortBy {
	FollowedAt = "followedAt",
	CreatedAt = "createdAt",
	UpdatedAt = "updatedAt",
	LastUploadedChapterAt = "lastUploadedChapterAt",
	Title = "title",
	Year = "year",
	Rating = "rating",
	Views = "views",
	Follows = "follows",
	Chapters = "chapters"
}

export const SortByText: Record<SortBy, string> = {
	[SortBy.FollowedAt]: "Followed Date",
	[SortBy.CreatedAt]: "Created Date",
	[SortBy.UpdatedAt]: "Updated Date",
	[SortBy.LastUploadedChapterAt]: "Uploaded Date",
	[SortBy.Title]: "Title",
	[SortBy.Year]: "Year",
	[SortBy.Rating]: "Rating",
	[SortBy.Views]: "Views",
	[SortBy.Follows]: "Follows",
	[SortBy.Chapters]: "Chapters"
};
