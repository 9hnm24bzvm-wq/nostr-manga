import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<Record<string, Record<string, null>>>(
		`${PUBLIC_API_URL}/user/history/manga${searchParams.toString()}`
	);
	http.setWithCredentials(true);
	return http.GET();
};

export const List = (mangaId: string) => {
	const http = new HTTP<Record<string, Record<string, null>>>(`${PUBLIC_API_URL}/user/history/manga/${mangaId}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Feed = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterHistoryFeedResponse>(
		`${PUBLIC_API_URL}/user/history/manga/feed${searchParams.toString()}`
	);
	http.setWithCredentials(true);
	return http.GET();
};

export interface UpdateChapterHistoryBatchRequest {
	read_ids?: string[];
	unread_ids?: string[];
}

export const UpdateBatch = (mangaId: string, payload: UpdateChapterHistoryBatchRequest) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/history/manga/${mangaId}`, payload);
	http.setWithCredentials(true);
	return http.POST();
};
