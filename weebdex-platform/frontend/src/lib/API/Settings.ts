import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export interface UpdateSettingsRequest {
	items_per_page?: string;
	languages?: string[];
	show_safe?: boolean;
	show_suggestive?: boolean;
	show_erotic?: boolean;
	show_hentai?: boolean;
	tags?: string[];
	translated_languages?: string[];
	uploaders?: string[];
	groups?: string[];
	data_saver?: boolean;
	version: number;
}

export const Update = (payload: UpdateSettingsRequest) => {
	const http = new HTTP<UserPreferences>(`${PUBLIC_API_URL}/settings`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};
