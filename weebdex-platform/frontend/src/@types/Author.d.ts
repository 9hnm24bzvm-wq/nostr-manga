declare global {
	interface Author {
		id: string;

		created_at: string;
		updated_at: string;

		name: string;
		description?: string;

		twitter?: string;
		pixiv?: string;
		fanbox?: string;
		fantia?: string;
		skeb?: string;
		youtube?: string;
		website?: string;

		locked?: boolean;
		version: number;
	}
}
export {};
