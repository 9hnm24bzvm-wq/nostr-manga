import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";

export const canDelete = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("cover.delete");
