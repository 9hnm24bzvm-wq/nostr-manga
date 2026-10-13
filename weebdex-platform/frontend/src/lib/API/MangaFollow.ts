import { PUBLIC_API_URL } from "$env/static/public";
import { Constants, HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<MangaFollowFeedResponse>(`${PUBLIC_API_URL}/user/follows/manga${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const LatestFeed = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterFeedResponse>(
		`${PUBLIC_API_URL}/user/follows/chapter/feed${searchParams.toString()}`
	);
	http.setWithCredentials(true);
	return http.GET();
};

export const LatestUpdates = (searchParams: URLSearchParams) => {
	const http = new HTTP<MappedChapterFeedResponse>(
		`${PUBLIC_API_URL}/user/follows/chapter/updates${searchParams.toString()}`
	);
	http.setWithCredentials(true);
	return http.GET();
};

interface UpdateMangaFollowRequest {
	status: Constants.Manga.ReadingStatus;
	notification?: boolean;
}

export const Update = (id: string, payload: UpdateMangaFollowRequest) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/manga/${id}`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export const Unfollow = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/manga/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const Import = (payload: {
	weebdex?: Record<string, Constants.Manga.ReadingStatus>;
	myanimelist?: Record<string, Constants.Manga.ReadingStatus>;
}) => {
	const http = new HTTP<void>(`${PUBLIC_API_URL}/user/follows/manga/import`, payload);
	http.setWithCredentials(true);
	return http.POST();
};
