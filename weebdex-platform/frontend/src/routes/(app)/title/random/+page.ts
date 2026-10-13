import { API } from "$lib";
import { prefs } from "$lib/stores";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async () => {
	const q = new URLSearchParams();
	prefs.applyMangaPreferences(q);
	q.set("hasChapters", "1");

	const res = await API.Manga.Random(q);
	if (res && res.status !== StatusCodes.OK) {
		error(res.status);
	}
	return {
		manga: res?.body
	};
};
