import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<ScanlationGroupFeedResponse>(`${PUBLIC_API_URL}/group${searchParams.toString()}`);
	return http.GET();
};

export const Get = (id: string) => {
	const http = new HTTP<ScanlationGroup>(`${PUBLIC_API_URL}/group/${id}`);
	return http.GET();
};

export interface CreateGroupRequest {
	name: string;
	description?: string;

	contact_email?: string;
	discord?: string;
	mangaupdates?: string;
	twitter?: string;
	website?: string;
}

export const Create = (payload: CreateGroupRequest) => {
	const http = new HTTP<ScanlationGroup>(`${PUBLIC_API_URL}/group`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export interface UpdateGroupRequest {
	name?: string;
	description?: string;

	contact_email?: string;
	discord?: string;
	mangaupdates?: string;
	twitter?: string;
	website?: string;

	locked?: boolean;
	inactive?: boolean;

	focused_languages?: string[];
	leader?: string;
	officers?: string[];
	members?: string[];

	version: number;
}

export const Update = (id: string, payload: UpdateGroupRequest) => {
	const http = new HTTP<ScanlationGroup>(`${PUBLIC_API_URL}/group/${id}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const Delete = (id: string) => {
	const http = new HTTP<void>(`${PUBLIC_API_URL}/group/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const Follows = (searchParams: URLSearchParams) => {
	const http = new HTTP<GroupFollowFeedResponse>(`${PUBLIC_API_URL}/user/follows/groups${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Follow = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);
	http.setWithCredentials(true);
	return http.POST();
};

export const Unfollow = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const GetFollowStatus = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/user/follows/groups/${id}`);
	http.setWithCredentials(true);
	return http.GET();
};
