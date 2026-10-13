import { Permission } from "$lib";
import API from "$lib/API";
import { Session } from "$lib/stores";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	if (!(await Session.isValid())) {
		error(StatusCodes.FORBIDDEN);
	}

	const res = await API.Chapter.Get(params.id);
	if (res?.status === StatusCodes.OK && res.body) {
		if (!Permission.Chapter.canUpdate(res.body)) {
			error(StatusCodes.FORBIDDEN);
		}
	} else if (res) error(res.status);
	else error(StatusCodes.INTERNAL_SERVER_ERROR);

	return {
		id: params.id,
		chapter: res?.body
	};
};
