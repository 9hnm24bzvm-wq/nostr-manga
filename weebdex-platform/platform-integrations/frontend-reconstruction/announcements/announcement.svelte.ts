import { announcementAPI, type Announcement } from "./announcement-client";
const announcements = $state<Announcement[]>([]);
export const announcementState = $state({ loading: false, error: "" });
let refreshQueued = false;
export async function refreshPinnedAnnouncements() {
	if (announcementState.loading) { refreshQueued = true; return; }
	announcementState.loading = true; announcementState.error = "";
	try {
		const feed = await announcementAPI.list({ pinned: true, limit: 100, page: 1, order: "desc" });
		announcements.splice(0, announcements.length, ...feed.data);
	} catch (error) { announcementState.error = error instanceof Error ? error.message : "Announcements could not be loaded"; }
	finally {
		announcementState.loading = false;
		if (refreshQueued) { refreshQueued = false; void refreshPinnedAnnouncements(); }
	}
}
if (typeof window !== "undefined") queueMicrotask(() => void refreshPinnedAnnouncements());
export default announcements;
