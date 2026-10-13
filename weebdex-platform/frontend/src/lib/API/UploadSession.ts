import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Get = () => {
	const http = new HTTP<{ id?: string }>(`${PUBLIC_API_URL}/upload`);
	http.setWithCredentials(true);
	return http.GET();
};

export const Begin = (mangaId: string, groups?: string[]) => {
	const http = new HTTP<UploadSession>(`${PUBLIC_API_URL}/upload/begin`, { manga: mangaId, groups });
	http.setWithCredentials(true);
	return http.POST();
};

export const BeginEdit = (
	chapterId: string,
	payload: {
		replace?: boolean;
		version: number;
	}
) => {
	const http = new HTTP<UploadSession>(`${PUBLIC_API_URL}/upload/begin/${chapterId}`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export const Delete = (id: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/upload/${id}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const Commit = (id: string, payload: { draft: ChapterDraft; page_order: string[] }) => {
	const http = new HTTP<Chapter>(`${PUBLIC_API_URL}/upload/${id}/commit`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export const DeleteFile = (sessionId: string, fileId: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/upload/${sessionId}/${fileId}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const DeleteFiles = (sessionId: string, fileIdList: string[]) => {
	const http = new HTTP(`${PUBLIC_API_URL}/upload/${sessionId}/batch`, fileIdList);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const CheckApprovalRequired = (language: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/upload/check-approval-required`, { language });
	http.setWithCredentials(true);
	return http.POST();
};
