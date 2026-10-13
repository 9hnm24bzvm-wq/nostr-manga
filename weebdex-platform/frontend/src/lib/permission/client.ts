import { Session } from "$lib/stores";

export const canCreate = () => Session.hasPerm("client.create");
export const canUpdate = () => Session.hasPerm("self.client.update");
export const canDelete = () => Session.hasPerm("self.client.delete");
