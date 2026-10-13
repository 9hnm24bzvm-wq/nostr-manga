import API from "$lib/API/index.js";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	const res = await API.Thread.Get(params.id);
	if (res && res.status !== StatusCodes.OK) {
		error(res.status);
	}

	return {
		id: params.id,
		thread: res?.body
	};
};
