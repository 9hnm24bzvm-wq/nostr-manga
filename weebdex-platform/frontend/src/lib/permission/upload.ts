import { Session } from "$lib/stores";

export const canCommit = () => Session.hasPerm("self.chapter.update");
export const canUpdateFiles = () => Session.hasPerm("self.chapter.update");
export const canDelete = () => Session.hasPerm("self.chapter.update");
