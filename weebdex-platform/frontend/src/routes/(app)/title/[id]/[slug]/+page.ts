import { API, Permission } from "$lib";
import { Session } from "$lib/stores/index.js";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ url, params }) => {
	const isDraft = url.searchParams.get("draft") === "true";
	if (isDraft && !(await Session.isValid())) {
		error(StatusCodes.FORBIDDEN);
	}

	const res = await (isDraft ? API.Manga.GetDraft(params.id) : API.Manga.Get(params.id));
	if (res?.status === StatusCodes.OK && res.body) {
		if (isDraft && !Permission.Manga.canUpdate(res.body)) {
			error(StatusCodes.FORBIDDEN);
		}
	} else if (res) error(res.status);
	else error(StatusCodes.INTERNAL_SERVER_ERROR);

	return {
		id: params.id,
		manga: res?.body
	};
};
