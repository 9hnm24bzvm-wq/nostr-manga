export enum Group {
	Content = "content",
	Format = "format",
	Genre = "genre",
	Theme = "theme",
	Author = "author",
	Tag = "tag"
}

export const GroupText: Record<string, string> = {
	[Group.Content]: "Content",
	[Group.Format]: "Format",
	[Group.Genre]: "Genre",
	[Group.Theme]: "Theme",
	[Group.Author]: "Author",
	[Group.Tag]: "Tag"
};
