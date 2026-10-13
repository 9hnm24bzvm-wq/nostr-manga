import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const User = (id: string) => {
	const http = new HTTP<UserStats>(`${PUBLIC_API_URL}/user/${id}/statistics`);
	return http.GET();
};

export const Manga = (id: string) => {
	const http = new HTTP<MangaStats>(`${PUBLIC_API_URL}/manga/${id}/statistics`);
	return http.GET();
};

export const Chapter = (id: string) => {
	const http = new HTTP<ChapterStats>(`${PUBLIC_API_URL}/chapter/${id}/statistics`);
	return http.GET();
};

export const Group = (id: string) => {
	const http = new HTTP<ScanlationGroupStats>(`${PUBLIC_API_URL}/group/${id}/statistics`);
	return http.GET();
};

export const Thread = (id: string) => {
	const http = new HTTP<ThreadStats>(`${PUBLIC_API_URL}/thread/${id}/statistics`);
	return http.GET();
};
