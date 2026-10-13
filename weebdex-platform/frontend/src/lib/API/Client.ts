import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const List = () => {
	const http = new HTTP<APIClient[]>(`${PUBLIC_API_URL}/client`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Create = (name: string) => {
	const http = new HTTP<APIClient>(`${PUBLIC_API_URL}/client`, { name });
	http.setWithCredentials(true);
	return http.POST();
};

export const Update = (id: string, name: string) => {
	const http = new HTTP<APIClient>(`${PUBLIC_API_URL}/client/${id}`, { name });
	http.setWithCredentials(true);
	return http.POST();
};

export const Delete = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/client/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const RegenerateSecret = (id: string) => {
	const http = new HTTP<APIClient>(`${PUBLIC_API_URL}/client/${id}/secret`);
	http.setWithCredentials(true);
	return http.POST();
};
