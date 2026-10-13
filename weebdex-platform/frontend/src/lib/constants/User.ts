export enum Role {
	Sysop = "sysop",
	System = "system",
	Staff = "staff",
	Moderator = "moderator",
	Banned = "banned",
	Restricted = "restricted",
	User = "user",
	Verified = "verified",
	Contributor = "contributor",
	PowerUploader = "power_uploader",
	GroupMember = "group_member",
	GroupOfficer = "group_officer",
	GroupLeader = "group_leader"
}

export const RoleText: Record<string, string> = {
	[Role.Sysop]: "Sysop",
	[Role.System]: "System",
	[Role.Staff]: "Staff",
	[Role.Moderator]: "Moderator",
	[Role.Banned]: "Banned",
	[Role.Restricted]: "Restricted",
	[Role.User]: "User",
	[Role.Verified]: "Verified",
	[Role.Contributor]: "Contributor",
	[Role.PowerUploader]: "Power Uploader",
	[Role.GroupMember]: "Group Member",
	[Role.GroupOfficer]: "Group Officer",
	[Role.GroupLeader]: "Group Leader"
};

export const RoleOrder = [
	"sysop",
	"system",
	"staff",
	"moderator",
	"contributor",
	"power_uploader",
	"group_leader",
	"group_officer",
	"group_member",
	"verified",
	"user",
	"restricted",
	"banned"
];
