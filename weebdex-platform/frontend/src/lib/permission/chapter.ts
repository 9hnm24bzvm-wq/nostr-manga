import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";
import { get } from "svelte/store";

export const canCreate = () => Session.hasPerm("chapter.create");

export const canUpdate = (chapter: Chapter) => {
	const session = get(Session);
	if (!session) return false;

	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("chapter.update")) {
		return true;
	} else if (!chapter.deleted_at && Session.hasPerm("self.chapter.update")) {
		if (chapter.relationships.uploader?.id === session.identity.id) {
			return true;
		}
		return chapter.relationships.groups?.some(group =>
			session.groups?.some(
				g =>
					g.id === group.id &&
					g.relationships?.members?.some(m => m.id === session.identity.id && (m.is_leader || m.is_officer))
			)
		);
	}
	return false;
};

export const canClaim = (chapter: Chapter) => {
	const session = get(Session);
	if (!session) return;
	return (
		Session.hasPerm("self.chapter.update") &&
		chapter.relationships.uploader?.id !== session.identity.id &&
		chapter.relationships.uploader?.name !== "system" &&
		chapter.relationships.groups?.some(group =>
			session.groups?.some(
				g => g.id === group.id && g.relationships?.members?.some(m => m.id === session.identity.id && m.is_leader)
			)
		)
	);
};

export const canRemove = (chapter: Chapter) => {
	const session = get(Session);
	if (!session) return false;

	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("chapter.remove")) {
		return true;
	} else if (!chapter.deleted_at && Session.hasPerm("self.chapter.remove")) {
		if (chapter.relationships.uploader?.id === session.identity.id) {
			return true;
		}
		return chapter.relationships.groups?.some(group =>
			session.groups?.some(
				g => g.id === group.id && g.relationships?.members?.some(m => m.id === session.identity.id && m.is_leader)
			)
		);
	}
	return false;
};

export const canDelete = () =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("chapter.delete");
