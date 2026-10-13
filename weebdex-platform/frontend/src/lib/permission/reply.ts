import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";
import { get } from "svelte/store";

export const canCreate = () => Session.hasPerm("reply.create");

export const canUpdate = (reply: Reply) => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("reply.update")) {
		return true;
	}
	return reply.relationships.author?.id === get(Session)?.identity.id && Session.hasPerm("self.reply.update");
};

export const canRemove = (reply: Reply) => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("reply.remove")) {
		return true;
	}
	// TODO: isremovable
	return reply.relationships.author?.id === get(Session)?.identity.id && Session.hasPerm("self.reply.remove");
};

export const canDelete = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("reply.delete");
