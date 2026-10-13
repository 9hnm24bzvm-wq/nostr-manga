import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<TagFeedResponse>(`${PUBLIC_API_URL}/manga/tag${searchParams.toString()}`);
	return http.GET();
};
