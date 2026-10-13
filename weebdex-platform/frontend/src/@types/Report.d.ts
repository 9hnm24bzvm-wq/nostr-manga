import type { Constants } from "$lib";

declare global {
	interface Report {
		id: string;

		created_at: string;
		updated_at: string;

		status: Constants.Report.Status;
		body?: string;
		notes?: string;

		relationships: {
			submitter: User;
			reason: ReportReason;

			user?: User;
			manga?: Manga;
			author?: Author;
			chapter?: Chapter;
			group?: ScanlationGroup;
			thread?: Thread;
			reply?: Reply;
		};
	}

	interface ReportReason {
		id: string;
		category: Constants.EntityType;
		reason: string;
		details_required: boolean;
	}
}
export {};
