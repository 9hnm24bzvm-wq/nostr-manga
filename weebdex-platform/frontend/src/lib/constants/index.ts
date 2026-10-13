import * as Error from "./Error";
import * as Manga from "./Manga";
import * as MangaFollow from "./MangaFollow";
import * as Reader from "./Reader";
import * as Report from "./Report";
import * as Tag from "./Tag";
import * as Thread from "./Thread";
import * as User from "./User";
export * as Error from "./Error";
export * as Manga from "./Manga";
export * as MangaFollow from "./MangaFollow";
export * as Reader from "./Reader";
export * as Report from "./Report";
export * as Tag from "./Tag";
export * as Thread from "./Thread";
export * as User from "./User";

export enum EntityType {
	User = "user",
	Manga = "manga",
	Author = "author",
	Chapter = "chapter",
	ScanlationGroup = "scanlation_group",
	Thread = "thread",
	Reply = "reply"
}

export const EntityTypeText: Record<string, string> = {
	[EntityType.User]: "User",
	[EntityType.Manga]: "Manga",
	[EntityType.Author]: "Author",
	[EntityType.Chapter]: "Chapter",
	[EntityType.ScanlationGroup]: "Scanlation Group",
	[EntityType.Thread]: "Thread",
	[EntityType.Reply]: "Reply"
};

export enum State {
	Draft = "draft",
	Submitted = "submitted",
	Published = "published",
	Rejected = "rejected"
}

export const StateText: Record<string, string> = {
	[State.Draft]: "Draft",
	[State.Submitted]: "Submitted",
	[State.Published]: "Published",
	[State.Rejected]: "Rejected"
};

export const MangaGridBreakpoints = {
	0: 7,
	1340: 6,
	1150: 5,
	960: 4,
	770: 3,
	580: 2
};

export const MangaPageGridBreakpoints = {
	0: 5,
	1367: 4,
	1180: 3,
	920: 2,
	767: 3,
	648: 2
};

export const MangaSliderBreakpoints = {
	1340: { slidesPerView: 7 },
	1150: { slidesPerView: 6 },
	960: { slidesPerView: 5 },
	770: { slidesPerView: 4 },
	580: { slidesPerView: 3 }
};

export default {
	MangaGridBreakpoints,
	MangaSliderBreakpoints,

	EntityType,
	State,

	Manga,
	MangaFollow,
	Report,
	Thread,
	Error,
	Tag,
	Reader,
	User
};
