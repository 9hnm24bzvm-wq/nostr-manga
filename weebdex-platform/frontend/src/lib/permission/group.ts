import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";
import { get } from "svelte/store";

export const canCreate = () => {
	return Session.hasPerm("scanlation_group.create");
};

export const canUpdate = (group: ScanlationGroup) => {
	const session = get(Session);
	if (!session) return false;

	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("scanlation_group.update")) {
		return true;
	} else if (Session.hasPerm("self.scanlation_group.update")) {
		return !!group.relationships?.members?.some(member => member.id === session.identity.id && member.is_leader);
	}
	return false;
};

export const canUpdateName = (): boolean => {
	return Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("scanlation_group.update");
};

export const canDelete = () => {
	return Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("scanlation_group.delete");
};
