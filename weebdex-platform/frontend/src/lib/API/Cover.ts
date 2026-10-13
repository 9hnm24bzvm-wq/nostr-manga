import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const List = (mangaId: string) => {
	const http = new HTTP<Cover[]>(`${PUBLIC_API_URL}/manga/${mangaId}/covers`);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<Cover>(`${PUBLIC_API_URL}/cover/${id}`);
	return http.GET();
};

export const Upload = (mangaId: string, payload: FormData) => {
	const http = new HTTP<Cover & { error: string }>(`${PUBLIC_API_URL}/cover/${mangaId}`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export interface UpdateCoverRequest {
	volume?: string;
	description?: string;
	language?: string;
	main?: boolean;
	version: number;
}

export const Update = (id: string, payload: UpdateCoverRequest) => {
	const http = new HTTP<Cover>(`${PUBLIC_API_URL}/cover/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Delete = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/cover/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};
