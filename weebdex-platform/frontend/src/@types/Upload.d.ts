declare global {
	interface UploadSession {
		id: string;
		created_at: string;
		updated_at: string;
		version: number;
		is_committed: bool;
		is_processed: bool;
		is_deleted: bool;
		relationships: {
			manga?: Manga;
			chapter?: Chapter | ChapterDraft;
			uploader?: User;
			groups?: ScanlationGroup[];
			files?: UploadSessionFile[];
		};
	}

	interface UploadSessionFile {
		id: string;
		file_hash: string;
		file_name: string;
		file_ext: string;
		file_size: string;

		original_file_name?: string;
		error?: string;

		file?: File;
		thumbnail?: string;
	}

	interface ChapterDraft {
		volume?: string;
		chapter?: string;
		title?: string;
		language?: string;
	}
}
export {};
