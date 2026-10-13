import { State } from "$lib/constants";
import { Role } from "$lib/constants/User";
import { Session } from "$lib/stores";
import { get } from "svelte/store";

export const canCreate = () => Session.hasPerm("manga.create");

export const canUpdate = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.update")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.update");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canRemove = (): boolean =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.remove");

export const canDelete = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.delete")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at && manga.state != State.Published) {
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditTitles = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.title.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.title.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditTags = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.tag.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.tag.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditAuthors = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.author.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.author.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditArtists = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.artist.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.artist.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditLinks = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.link.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.link.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canEditRelations = (manga?: Manga): boolean => {
	if (!Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && (manga?.locked || !manga?.deleted_at)) {
		return false;
	}
	return Session.hasPerm("manga.relation.edit");
};

export const canEditCovers = (manga: Manga): boolean => {
	if (Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.cover.edit")) {
		return true;
	} else if (!manga.locked && !manga.deleted_at) {
		if (manga.state === State.Published) {
			return Session.hasPerm("manga.cover.edit");
		}
		return manga.relationships.submitter?.id === get(Session)?.identity.id && Session.hasPerm("manga.create");
	}
	return false;
};

export const canUpdateState = (): boolean =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.state.update");

export const canUpdateLock = (): boolean =>
	Session.hasRole(Role.Sysop, Role.Staff, Role.Moderator) && Session.hasPerm("manga.lock.update");
