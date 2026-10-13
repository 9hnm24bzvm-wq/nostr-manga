import API from "$lib/API";
import Mutex from "$lib/Mutex";
import { StatusCodes } from "http-status-codes";
import { get, writable } from "svelte/store";

const store = writable<CheckSessionResponse | undefined>();
const mutex = new Mutex();

const initialize = () =>
	mutex.exec(async () => {
		if (get(store)) return;
		const res = await API.Auth.Check();
		if (res?.status === StatusCodes.OK && res.body) {
			store.set(res.body);
		}
	});
initialize();

const isValid = () =>
	mutex.wait(() => {
		const session = get(store);
		return !!session?.identity?.id?.length && !!session?.identity?.username?.length;
	});

const hasPerm = (...perms: string[]) => {
	const session = get(store);
	if (!session) return false;
	return perms.some(perm => session.permissions?.includes(perm));
};

const hasRole = (...roles: string[]) => {
	const session = get(store);
	if (!session) return false;
	return roles.some(role => session.roles?.includes(role));
};

export default <
	typeof store & {
		initialize: typeof initialize;
		isValid: typeof isValid;
		hasPerm: typeof hasPerm;
		hasRole: typeof hasRole;
	}
>Object.assign(store, {
	initialize,
	isValid,
	hasPerm,
	hasRole
});
