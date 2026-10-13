import type { Constants } from "$lib";

declare global {
	interface Reply {
		id: number;
		reply_number: number;

		created_at: string;
		updated_at: string;

		state?: Constants.Thread.ReplyState;
		body?: string;
		version: number;

		relationships: {
			author?: User;
			stats: ReplyStats;
			thread?: Thread;

			vote_state?: -1 | 1;
		};
		_is_virtual?: boolean;
	}

	interface ReplyStats {
		up: number;
		down: number;
	}
}
export {};
