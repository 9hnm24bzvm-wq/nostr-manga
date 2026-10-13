declare global {
	interface ScanlationGroup {
		id: string;
		name: string;
		version: number;

		created_at?: string;
		updated_at?: string;

		description?: string;
		contact_email?: string;
		discord?: string;
		mangaupdates?: string;
		twitter?: string;
		website?: string;

		locked?: boolean;
		inactive?: boolean;

		relationships?: {
			members?: GroupMember[];
			stats?: ScanlationGroupStats;
			thread?: Thread;
		};
	}

	interface ScanlationGroupStats {
		chapters: number;
	}

	interface GroupMember extends User {
		is_leader?: boolean;
		is_officer?: boolean;
	}

	interface GroupFollow extends User {
		followed_at: string;
	}
}
export {};
