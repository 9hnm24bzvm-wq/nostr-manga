declare global {
	interface Feed<T> {
		total: number;
		limit: number;
		page: number;
		data: T[];
	}

	interface MappedFeed<T, M> extends Feed<T> {
		map: M;
	}
}
export {};
