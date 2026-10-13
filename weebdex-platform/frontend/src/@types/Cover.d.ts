declare global {
	interface Cover {
		id: string;

		created_at?: string;
		updated_at?: string;

		ext: string;
		dimensions: number[];

		volume: string;
		description?: string;
		language: string;
		version: int;

		relationships?: {
			uploader: User;
		};
	}
}
export {};
