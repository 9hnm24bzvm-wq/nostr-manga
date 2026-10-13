import { PUBLIC_API_URL } from "$env/static/public";
import { HTTP } from "$lib";

export const Get = (id: string) => {
	const http = new HTTP<Thread>(`${PUBLIC_API_URL}/thread/${id}`);
	return http.GET();
};

export const GetThreadVote = (threadId: string) => {
	const http = new HTTP<{ vote: number }>(`${PUBLIC_API_URL}/thread/${threadId}/vote`);
	http.setWithCredentials(true);
	return http.GET();
};

export const VoteThread = (threadId: string, vote: -1 | 1) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/vote`, { vote });
	http.setWithCredentials(true);
	return http.POST();
};

export const UnvoteThread = (threadId: string) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/vote`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const Replies = (threadId: string, searchParams: URLSearchParams) => {
	const http = new HTTP<ReplyFeedResponse>(`${PUBLIC_API_URL}/thread/${threadId}/replies${searchParams.toString()}`);
	http.setWithCredentials(true);
	return http.GET();
};

export const GetReply = (threadId: string, replyId: string) => {
	const http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`);
	http.setWithCredentials(true);
	return http.GET();
};

export interface CreateReplyRequest {
	parent_id?: number;
	body: string;
}

export const CreateReply = (threadId: string, payload: CreateReplyRequest) => {
	const http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies`, payload);
	http.setWithCredentials(true);
	return http.POST();
};

export const UpdateReply = (threadId: string, replyId: number, payload: { body: string; version: number }) => {
	const http = new HTTP<Reply>(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`, payload);
	http.setWithCredentials(true);
	return http.PUT();
};

export const RemoveReply = (threadId: string, replyId: number) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const DeleteReply = (threadId: string, replyId: number) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/hard`);
	http.setWithCredentials(true);
	return http.DELETE();
};

export const VoteReply = (threadId: string, replyId: number, vote: -1 | 1) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/vote`, { vote });
	http.setWithCredentials(true);
	return http.POST();
};

export const UnvoteReply = (threadId: string, replyId: number) => {
	const http = new HTTP(`${PUBLIC_API_URL}/thread/${threadId}/replies/${replyId}/vote`);
	http.setWithCredentials(true);
	return http.DELETE();
};
