import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<UserFeedResponse>(`${PUBLIC_API_URL}/user${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<User>(`${PUBLIC_API_URL}/user/${id}`);
	return http.GET();
};

export interface UpdateUserRequest {
	version: number;
	description?: string;
	discord?: string;
	twitter?: string;
	website?: string;
}

export const Update = (id: string, payload: UpdateUserRequest) => {
	const http = new HTTP<User>(`${PUBLIC_API_URL}/user/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Follows = (searchParams: URLSearchParams) => {
	const http = new HTTP<UserFollowFeedResponse>(`${PUBLIC_API_URL}/user/follows/users${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Follow = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);
	http.setWithCredentials(true);
	return http.POST();
};

export const Unfollow = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const GetFollowStatus = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/users/${id}`);
	http.setWithCredentials(true);
	return http.GET();
};
