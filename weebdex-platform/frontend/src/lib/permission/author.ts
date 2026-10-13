import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";

export const canCreate = () => Session.hasPerm("author.create");

export const canUpdate = (author: Author) => {
	if (!Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && author.locked) {
		return false;
	}
	return Session.hasPerm("author.update");
};

export const canUpdateLock = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("author.lock.update");

export const canDelete = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("author.delete");
