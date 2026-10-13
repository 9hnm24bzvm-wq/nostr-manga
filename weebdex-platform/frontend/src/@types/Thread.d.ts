declare global {
	interface Thread {
		id: string;

		created_at: string;
		updated_at: string;
		deleted_at?: string;

		subject: string;
		body: string;
		locked: boolean;

		relationships: {
			author: User;

			user?: User;
			manga?: Manga;
			chapter?: Chapter;
			group?: Group;

			stats: ThreadStats;
		};
	}
	interface ThreadStats {
		replies: number;
		up: number;
		down: number;
	}
}
export {};
