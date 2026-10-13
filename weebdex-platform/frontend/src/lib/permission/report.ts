import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";

export const canCreate = () => Session.hasPerm("report.create");

export const canUpdate = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("report.update");

export const canDelete = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("report.delete");
