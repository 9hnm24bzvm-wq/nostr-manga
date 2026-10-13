import { API } from "$lib";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	const res = await API.Manga.Get(params.id);
	if (res && res.status !== StatusCodes.OK) {
		error(res.status);
	}
	return {
		id: params.id,
		manga: res?.body
	};
};
