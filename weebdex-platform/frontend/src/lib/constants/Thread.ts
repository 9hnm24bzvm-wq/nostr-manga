export enum ReplyState {
	Pending = "pending",
	Active = "active",
	Locked = "locked",
	Hidden = "hidden",
	Flagged = "flagged",
	Archived = "archived",
	DeletedByUser = "deleted_by_user",
	DeletedByModerator = "deleted_by_moderator",
	DeletedBySystem = "deleted_by_system"
}

export const ReplyRemovedStateText: Record<string, string> = {
	[ReplyState.DeletedByUser]: "This comment was removed by the user.",
	[ReplyState.DeletedByModerator]: "This comment was removed by a moderator.",
	[ReplyState.DeletedBySystem]: "This comment was automatically removed by the system."
};
