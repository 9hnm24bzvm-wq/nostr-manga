import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<TagFeedResponse>(`${PUBLIC_API_URL}/author${searchParams.toString()}`);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<Author>(`${PUBLIC_API_URL}/author/${id}`);
	return http.GET();
};

export const Create = (name: string) => {
	const http = new HTTP<Author>(`${PUBLIC_API_URL}/author`, { name });
	http.setWithCredentials(true);
	return http.POST();
};

export interface UpdateAuthorRequest {
	name?: string;
	description?: string;

	twitter?: string;
	pixiv?: string;
	fanbox?: string;
	fantia?: string;
	skeb?: string;
	youtube?: string;
	website?: string;

	locked?: boolean;
	version: number;
}

export const Update = (id: string, payload: UpdateAuthorRequest) => {
	const http = new HTTP<Author>(`${PUBLIC_API_URL}/author/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Delete = (id: string) => {
	const http = new HTTP<Author>(`${PUBLIC_API_URL}/author/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};
