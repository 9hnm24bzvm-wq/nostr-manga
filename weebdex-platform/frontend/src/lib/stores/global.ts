import { API, Constants, Utils } from "$lib";
import { IsMobile, PersistentStore } from "$lib/hooks";
import { StatusCodes } from "http-status-codes";
import { get, writable } from "svelte/store";

export const from = writable<URL | undefined>();
export const tagList = writable<Record<string, Tag[]> | undefined>();
export const populateTagList = async () => {
	if (get(tagList)?.length) {
		return;
	}

	const res = await API.Tag.Search(new URLSearchParams());
	if (res?.status === StatusCodes.OK && res.body?.data) {
		tagList.set(Utils.groupTags(res.body.data));
	}
};

export const isMobile = new IsMobile();
export const isShowSearchDialog = writable(false);
export const isShowSidebar = new PersistentStore("cb10ctj7", false);
export const mangaView = new PersistentStore("nrs2qcav", "grid");
export const readerTab = new PersistentStore("ho5r3tmc", "settings");
export const commentsVisibility = new PersistentStore("whyxk2oq", Constants.Reader.CommentsVisibility.Collapsed);
export const skipEndOfChapterPage = new PersistentStore("obw66lfdqo", false);
