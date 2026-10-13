export enum ContentRating {
	Safe = "safe",
	Suggestive = "suggestive",
	Erotica = "erotica",
	Pornographic = "pornographic"
}

export const ContentRatingText: Record<string, string> = {
	[ContentRating.Safe]: "Safe",
	[ContentRating.Suggestive]: "Suggestive",
	[ContentRating.Erotica]: "Erotica",
	[ContentRating.Pornographic]: "Pornographic"
};

export enum Demographic {
	Shounen = "shounen",
	Shoujo = "shoujo",
	Josei = "josei",
	Seinen = "seinen",
	None = "none"
}

export const DemographicText = {
	[Demographic.Shounen]: "Shounen",
	[Demographic.Shoujo]: "Shoujo",
	[Demographic.Josei]: "Josei",
	[Demographic.Seinen]: "Seinen",
	[Demographic.None]: "None"
};

export enum PublicationStatus {
	Ongoing = "ongoing",
	Completed = "completed",
	Hiatus = "hiatus",
	Cancelled = "cancelled"
}

export const PublicationStatusText = {
	[PublicationStatus.Ongoing]: "Ongoing",
	[PublicationStatus.Completed]: "Completed",
	[PublicationStatus.Hiatus]: "Hiatus",
	[PublicationStatus.Cancelled]: "Cancelled"
};

export enum ReadingStatus {
	Reading = "reading",
	PlanToRead = "plan_to_read",
	Completed = "completed",
	OnHold = "on_hold",
	ReReading = "re_reading",
	Dropped = "dropped"
}

export const ReadingStatusText: Record<string, string> = {
	[ReadingStatus.Reading]: "Reading",
	[ReadingStatus.PlanToRead]: "Plan to Read",
	[ReadingStatus.Completed]: "Completed",
	[ReadingStatus.OnHold]: "On Hold",
	[ReadingStatus.ReReading]: "Re-reading",
	[ReadingStatus.Dropped]: "Dropped"
};

export const ReadingStatusMapping: Record<number, ReadingStatus> = {
	0: ReadingStatus.Reading,
	1: ReadingStatus.PlanToRead,
	2: ReadingStatus.Completed,
	3: ReadingStatus.OnHold,
	4: ReadingStatus.ReReading,
	5: ReadingStatus.Dropped
};

export const ReadingStatusMappingReverse: Record<ReadingStatus, number> = {
	[ReadingStatus.Reading]: 0,
	[ReadingStatus.PlanToRead]: 1,
	[ReadingStatus.Completed]: 2,
	[ReadingStatus.OnHold]: 3,
	[ReadingStatus.ReReading]: 4,
	[ReadingStatus.Dropped]: 5
};

export enum RelationType {
	MainStory = "main_story",
	SideStory = "side_story",
	Prequel = "prequel",
	Sequel = "sequel",
	AdaptedFrom = "adapted_from",
	SpinOff = "spin_off",
	BasedOn = "based_on",
	Doujinshi = "doujinshi",
	Monochrome = "monochrome",
	Colored = "colored",
	Preserialization = "preserialization",
	Serialization = "serialization",
	AlternateStory = "alternate_story",
	AlternateVersion = "alternate_version",
	SameFranchise = "same_franchise",
	SharedUniverse = "shared_universe"
}

export const RelationTypeText = {
	[RelationType.MainStory]: "Main Story",
	[RelationType.SideStory]: "Side Story",
	[RelationType.Prequel]: "Prequel",
	[RelationType.Sequel]: "Sequel",
	[RelationType.AdaptedFrom]: "Adapted From",
	[RelationType.SpinOff]: "Spin Off",
	[RelationType.BasedOn]: "Based On",
	[RelationType.Doujinshi]: "Doujinshi",
	[RelationType.Monochrome]: "Monochrome",
	[RelationType.Colored]: "Colored",
	[RelationType.Preserialization]: "Preserialization",
	[RelationType.Serialization]: "Serialization",
	[RelationType.AlternateStory]: "Alternate Story",
	[RelationType.AlternateVersion]: "Alternate Version",
	[RelationType.SameFranchise]: "Same Franchise",
	[RelationType.SharedUniverse]: "Shared Universe"
};

export enum SortBy {
	CreatedAt = "createdAt",
	UpdatedAt = "updatedAt",
	LastUploadedChapterAt = "lastUploadedChapterAt",
	Relevance = "relevance",
	Title = "title",
	Year = "year",
	Rating = "rating",
	Views = "views",
	Follows = "follows",
	Chapters = "chapters"
}

export const SortByText: Record<SortBy, string> = {
	[SortBy.CreatedAt]: "Created Date",
	[SortBy.UpdatedAt]: "Updated Date",
	[SortBy.LastUploadedChapterAt]: "Uploaded Date",
	[SortBy.Relevance]: "Relevance",
	[SortBy.Title]: "Title",
	[SortBy.Year]: "Year",
	[SortBy.Rating]: "Rating",
	[SortBy.Views]: "Views",
	[SortBy.Follows]: "Follows",
	[SortBy.Chapters]: "Chapters"
};

export enum DraftSortBy {
	CreatedAt = "createdAt",
	UpdatedAt = "updatedAt",
	Title = "title",
	Year = "year"
}

export const DraftSortByText = {
	[DraftSortBy.CreatedAt]: "Created Date",
	[DraftSortBy.UpdatedAt]: "Updated Date",
	[DraftSortBy.Title]: "Title",
	[DraftSortBy.Year]: "Year"
};
