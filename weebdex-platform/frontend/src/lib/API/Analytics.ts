import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Manga = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/analytics/manga/${id}/views`);
	return http.POST();
};

export const Chapter = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/analytics/chapter/${id}/views`);
	return http.POST();
};
