import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";
import { get } from "svelte/store";

export const canUpdate = (user: User) => {
	if (Session.hasRole(Role.Sysop, Role.Staff) && Session.hasPerm("user.update")) {
		return true;
	}
	return user.id === get(Session)?.identity.id && Session.hasPerm("self.user.update");
};
