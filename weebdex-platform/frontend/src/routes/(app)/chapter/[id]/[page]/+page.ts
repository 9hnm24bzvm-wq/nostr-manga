import API from "$lib/API/index.js";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	const res = await API.Chapter.Get(params.id);
	if (res && res.status !== StatusCodes.OK) {
		error(res.status);
	}

	if (res?.body) {
		res.body.relationships.thread.relationships.chapter = {
			...res.body,
			relationships: {
				...res.body.relationships,
				thread: null!
			}
		};
	}
	return {
		id: params.id,
		chapter: res?.body
	};
};
