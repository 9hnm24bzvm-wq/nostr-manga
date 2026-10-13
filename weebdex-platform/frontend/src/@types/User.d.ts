declare global {
	interface User {
		id: string;
		name: string;
		version: number;

		description?: string;
		discord?: string;
		twitter?: string;
		website?: string;

		roles?: string[];

		relationships?: UserRelationships;
	}

	interface UserRelationships {
		groups?: ScanlationGroup[];
		stats?: UserStats;
		thread?: Thread;
	}

	interface UserStats {
		chapters: number;
		messages: number;
		points: number;
	}

	interface UserFollow extends User {
		followed_at: string;
	}
}
export {};
