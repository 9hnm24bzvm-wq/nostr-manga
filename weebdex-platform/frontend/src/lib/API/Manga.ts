import { PUBLIC_API_URL } from "$env/static/public";
import { Constants, HTTP } from "$lib";

export interface SearchMangaRequest {
	reading_status?: string[];

	title?: string;
	yearFrom?: number;
	yearTo?: number;

	lang?: string[];
	langx?: string[];
	demographic?: Constants.Manga.Demographic[];
	status?: Constants.Manga.PublicationStatus[];
	contentRating?: Constants.Manga.ContentRating[];

	hasChapters?: boolean;
	availableTranslatedLang?: string[];

	tag?: string[];
	tmod?: string;
	tagx?: string[];
	txmod?: string;

	author?: string[];
	artists?: string[];

	limit: number;
	sort: Constants.Manga.SortBy | Constants.MangaFollow.SortBy;
	order: "asc" | "desc";
}

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<MangaFeedResponse>(`${PUBLIC_API_URL}/manga${searchParams.toString()}`);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/${id}`);
	return http.GET();
};

export const Random = (searchParams: URLSearchParams) => {
	const xhr = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/random${searchParams.toString()}`);
	return xhr.GET();
};

export const Trending = (searchParams: URLSearchParams) => {
	const xhr = new HTTP<MangaFeedResponse>(`${PUBLIC_API_URL}/manga/top${searchParams.toString()}`);
	return xhr.GET();
};

export const Relations = (id: string) => {
	const xhr = new HTTP<Relation[]>(`${PUBLIC_API_URL}/manga/${id}/relations`);
	return xhr.GET();
};

export const Recommendations = (id: string) => {
	const xhr = new HTTP<MangaFeedResponse>(`${PUBLIC_API_URL}/manga/${id}/recommendations`);
	return xhr.GET();
};

export const SearchDrafts = (searchParams: URLSearchParams) => {
	const http = new HTTP<MangaFeedResponse>(`${PUBLIC_API_URL}/manga/draft${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const GetDraft = (id: string) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/draft/${id}`);
	http.setWithCredentials(true);
	return http.GET();
};

export interface CreateMangaRequest {
	title: string;
	description?: string;
	year?: number;
	language: string;
	demographic?: Constants.Manga.Demographic;
	status: Constants.Manga.PublicationStatus;
	content_rating: Constants.Manga.ContentRating;

	last_volume?: string;
	last_chapter?: string;

	alt_titles?: Record<string, string[]>;
	tags?: string[];
	authors: string[];
	artists: string[];
	links?: Record<string, string>;
}

export const Create = (payload: CreateMangaRequest) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export interface UpdateMangaRequest extends Partial<CreateMangaRequest> {
	version: number;
}

export const Update = (id: string, payload: UpdateMangaRequest) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Commit = (id: string, version: number) => {
	const http = new HTTP<Manga & { error?: string }>(`${PUBLIC_API_URL}/manga/draft/${id}/commit`, { version });
	http.setWithCredentials(true);
	return http.POST();
};

export const Publish = (id: string, version: number) => {
	const http = new HTTP<Manga & { error?: string }>(`${PUBLIC_API_URL}/manga/draft/${id}/publish`, { version });
	http.setWithCredentials(true);
	return http.POST();
};

export const Reject = (id: string, version: number) => {
	const http = new HTTP<Manga & { error?: string }>(`${PUBLIC_API_URL}/manga/draft/${id}/reject`, { version });
	http.setWithCredentials(true);
	return http.POST();
};

export const Lock = (id: string) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/${id}/lock`);
	http.setWithCredentials(true);
	return http.POST();
};

export const Unlock = (id: string) => {
	const http = new HTTP<Manga>(`${PUBLIC_API_URL}/manga/${id}/unlock`);
	http.setWithCredentials(true);
	return http.POST();
};

export const Delete = (id: string) => {
	const http = new HTTP<void>(`${PUBLIC_API_URL}/manga/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export interface CreateMangaRelationRequest {
	related_id: string;
	type: Constants.Manga.RelationType;
}

export const CreateRelation = (id: string, payload: CreateMangaRelationRequest) => {
	const http = new HTTP<Relation>(`${PUBLIC_API_URL}/manga/${id}/relations`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export interface DeleteMangaRelationRequest {
	related_id: string;
}

export const DeleteRelation = (id: string, payload: DeleteMangaRelationRequest) => {
	const http = new HTTP<void>(`${PUBLIC_API_URL}/manga/${id}/relations`, payload);
	http.setWithCredentials(true);
	return http.DELETE();
};
