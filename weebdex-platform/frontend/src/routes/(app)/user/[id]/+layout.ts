import { API } from "$lib";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	const res = await API.User.Get(params.id);
	if (res && res.status !== StatusCodes.OK) {
		error(res.status);
	}

	if (res?.body) {
		if (res.body.relationships?.thread) {
			res.body.relationships.thread.relationships.user = {
				...res.body,
				relationships: {
					...res.body.relationships,
					thread: null!
				}
			};
		}
	}
	return {
		id: params.id,
		user: res?.body
	};
};
