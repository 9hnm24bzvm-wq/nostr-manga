import { Constants } from "$lib";
import { PersistentStore } from "$lib/hooks";
import isMobile from "is-mobile";
import { get } from "svelte/store";

export const prefs = (() => {
	const store = new PersistentStore<UserPreferences>("xzp0yv4s", {
		items_per_page: "42",
		languages: [],
		show_safe: true,
		show_suggestive: true,
		show_erotic: true,
		show_hentai: false,
		tags: [],
		translated_languages: [],
		uploaders: [],
		groups: [],
		version: 0
	});
	return Object.assign(store, {
		applyMangaPreferences(searchParams: URLSearchParams) {
			const tmp = new URLSearchParams(searchParams);
			["page", "limit"].forEach(k => tmp.delete(k));

			const prefs = get(store);
			if (!tmp.toString().length) {
				prefs.languages.forEach(code => searchParams.append("lang", code));
				prefs.tags.forEach(id => searchParams.append("tagx", id));
			}

			if (!tmp.has("contentRating")) {
				if (!(prefs.show_safe && prefs.show_suggestive && prefs.show_erotic && !prefs.show_hentai)) {
					if (prefs.show_safe) searchParams.append("contentRating", Constants.Manga.ContentRating.Safe);
					if (prefs.show_suggestive) searchParams.append("contentRating", Constants.Manga.ContentRating.Suggestive);
					if (prefs.show_erotic) searchParams.append("contentRating", Constants.Manga.ContentRating.Erotica);
					if (prefs.show_hentai) searchParams.append("contentRating", Constants.Manga.ContentRating.Pornographic);
				}
			}
		},
		applyChapterPreferences(searchParams: URLSearchParams, groupsOnly = false) {
			const prefs = get(store);
			if (!groupsOnly) {
				prefs.translated_languages.forEach(code => searchParams.append("tlang", code));
				prefs.uploaders.forEach(({ id }) => searchParams.append("uploaderx", id));
			}
			prefs.groups.forEach(({ id }) => searchParams.append("groupx", id));
		}
	});
})();

export const dataSaver = new PersistentStore<boolean>("s8qd0p4u", isMobile());

export const mangaPrefs = new PersistentStore<ReaderPreferences>("y8jz8w2m", {
	scale: isMobile() ? Constants.Reader.Scale.LimitWidth : Constants.Reader.Scale.LimitHeight,
	layout: Constants.Reader.Layout.Paged,
	direction: Constants.Reader.Direction.LTR,

	double: false,
	offset: 0,

	width: 100,
	gap: 0,
	scroll: 25,

	preload: 5,
	parallel: 3,

	tap_navigation: true,
	keyboard_navigation: true,
	scroll_navigation: false,
	auto_advance: true,

	panel: !isMobile(),
	panel_position: "right",

	version: 1
});

export const webtoonPrefs = new PersistentStore<ReaderPreferences>("g7pivu0g", {
	scale: Constants.Reader.Scale.LimitWidth,
	layout: Constants.Reader.Layout.Scroll,
	direction: Constants.Reader.Direction.LTR,

	double: false,
	offset: 0,

	width: 100,
	gap: 0,
	scroll: 25,

	preload: 5,
	parallel: 3,

	tap_navigation: true,
	keyboard_navigation: true,
	scroll_navigation: false,
	auto_advance: true,

	panel: !isMobile(),
	panel_position: "right",

	version: 1
});
