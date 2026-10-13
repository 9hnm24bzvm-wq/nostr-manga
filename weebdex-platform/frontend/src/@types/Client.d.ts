declare global {
	interface APIClient {
		id: string;

		created_at: string;
		updated_at: string;

		name: string;
		key?: string;
		version: number;
	}
}
export {};
