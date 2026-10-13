declare global {
	interface ChapterHistory extends Chapter {
		read_at: string;
	}
}
export {};
