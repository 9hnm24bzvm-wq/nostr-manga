import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Check = () => {
	const http = new HTTP<CheckSessionResponse>(`${PUBLIC_API_URL}/auth/check`);
	http.setWithCredentials(true);
	return http.GET();
};
