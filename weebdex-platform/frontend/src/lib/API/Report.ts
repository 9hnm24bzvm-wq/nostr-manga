import { PUBLIC_API_URL } from "$env/static/public";
import { Constants, HTTP } from "$lib";

export interface SearchReportRequest {
	category?: string;
	object?: string;
	reason?: string;
	status?: string;
	order?: "asc" | "desc";
}

export const Search = (searchParams: URLSearchParams) => {
	const http = new HTTP<ReportFeedResponse>(`${PUBLIC_API_URL}/report${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Reasons = (entity: Constants.EntityType) => {
	const http = new HTTP<ReportReason[]>(`${PUBLIC_API_URL}/report/reasons/${entity}`);
	return http.GET();
};

export interface CreateReportRequest {
	entity_type: Constants.EntityType;
	entity_id: string;
	reason_id: string;
	details?: string;
}

export const Create = (payload: CreateReportRequest) => {
	const http = new HTTP(`${PUBLIC_API_URL}/report`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export const Delete = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/report/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const Accept = (id: string, notes?: string) => {
	const http = new HTTP<Report>(`${PUBLIC_API_URL}/report/${id}/accept`, { notes });
	http.setWithCredentials(true);
	return http.POST();
};

export const Reject = (id: string, notes?: string) => {
	const http = new HTTP<Report>(`${PUBLIC_API_URL}/report/${id}/reject`, { notes });
	http.setWithCredentials(true);
	return http.POST();
};
