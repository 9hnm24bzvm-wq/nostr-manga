import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterFeedResponse>(`${PUBLIC_API_URL}/chapter${searchParams.toString()}`);
	return http.GET();
};

export const LatestUpdates = (searchParams: URLSearchParams) => {
	const http = new HTTP<ChapterFeedResponse>(`${PUBLIC_API_URL}/chapter/updates${searchParams.toString()}`);
	return http.GET();
};

export const LatestFeed = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterFeedResponse>(`${PUBLIC_API_URL}/chapter/feed${searchParams.toString()}`);
	return http.GET();
};

export const Trending = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterFeedResponse>(`${PUBLIC_API_URL}/chapter/top${searchParams.toString()}`);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<Chapter>(`${PUBLIC_API_URL}/chapter/${id}`);
	return http.GET();
};

export interface ListChaptersRequest {
	tlang?: string[];
	tlangx?: string[];
	uploader?: string[];
	uploaderx?: string[];
	group?: string[];
	groupx?: string[];

	limit?: number;
	page?: number;
	sort?: string;
	order?: string;
}

export const List = (mangaId: string, searchParams: URLSearchParams) => {
	const http = new HTTP<ChapterFeedResponse>(`${PUBLIC_API_URL}/manga/${mangaId}/chapters${searchParams.toString()}`);
	return http.GET();
};

export const Aggregate = (mangaId: string, searchParams: URLSearchParams) => {
	const http = new HTTP<ChapterAggregateResponse>(
		`${PUBLIC_API_URL}/manga/${mangaId}/aggregate${searchParams.toString()}`
	);
	return http.GET();
};

export interface UpdateChapterRequest {
	volume?: string;
	chapter?: string;
	title?: string;
	language?: string;
	groups?: string[];

	version: number;
}

export const Update = (id: string, payload: UpdateChapterRequest) => {
	const http = new HTTP<Chapter>(`${PUBLIC_API_URL}/chapter/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Claim = (id: string) => {
	const http = new HTTP<Chapter>(`${PUBLIC_API_URL}/chapter/${id}/claim`);
	http.setWithCredentials(true);
	return http.POST();
};

export const Delete = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/chapter/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};
