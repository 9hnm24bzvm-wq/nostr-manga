import { goto } from "$app/navigation";
import API from "$lib/API/index.js";
import { error } from "@sveltejs/kit";
import { StatusCodes } from "http-status-codes";

export const load = async ({ params }) => {
	const threadRes = await API.Thread.Get(params.id);
	if (threadRes && threadRes.status !== StatusCodes.OK) {
		error(threadRes.status);
	}

	if (threadRes?.body) {
		const replyRes = await API.Thread.GetReply(params.id, params.postId);
		if (replyRes?.status === StatusCodes.OK && replyRes.body?.reply_number) {
			const page = Math.ceil(
				(threadRes.body.relationships.stats.replies - Number(replyRes.body.reply_number) + 1) / 20
			);

			const q = new URLSearchParams();
			if (page > 1) q.set("page", page.toString());
			// eslint-disable-next-line svelte/no-navigation-without-resolve
			goto(`/forums/threads/${params.id}${q.toString()}#p${replyRes.body.id}`, { replaceState: true });
		}
	}
};
