// Source-backed HTTP replacement: never replay an ambiguous mutation.
import { StatusCodes } from "http-status-codes";
import { RateLimiter } from "limiter";
import { Error as UserVisibleError } from "$lib/constants/Error";

export class HTTPError extends UserVisibleError {
	constructor(public code: number, public error: string, title = "Request failed") {
		super(title, error);
	}
}
export interface HTTPResponse<T = void> {
	status: number;
	body?: T;
	headers?: Record<string, string | null>;
}
// Tokens are a refilling request budget, not permits held by an active request.
const limiter = new RateLimiter({ tokensPerInterval: 5, interval: "second" });
class HTTP<T, U = unknown> {
	private xhr: XMLHttpRequest;
	private timeout = 0;
	private timeoutMs = 60000;
	private maxAttempts = 3;
	private isAlreadyOpened = false;
	private cancelled = false;
	private finished = false;
	private cancelPending?: () => void;
	private deadlinePending?: () => void;
	private method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE" = "GET";
	private headers: Record<string, string> = {};
	constructor(private readonly url: string, private readonly body?: U) {
		this.xhr = new XMLHttpRequest();
	}
	private open() {
		this.xhr.open(this.method, this.url);
		for (const [key, value] of Object.entries(this.headers)) this.xhr.setRequestHeader(key, value);
	}
	private send(): Promise<HTTPResponse<T>> {
		this.isAlreadyOpened = true;
		return new Promise((resolve, reject) => {
			let attempts = 0;
			let terminal = false;
			let timer = 0;
			let sentMutation = false;
			const cleanup = () => {
				window.clearTimeout(timer);
				window.clearTimeout(this.timeout);
				this.xhr.removeEventListener("load", onLoad);
				this.xhr.removeEventListener("error", onTransportFailure);
				this.xhr.removeEventListener("timeout", onTransportFailure);
				this.xhr.removeEventListener("abort", onAbort);
				this.cancelPending = this.deadlinePending = undefined;
				this.finished = true;
			};
			const fail = (reason: string) => {
				if (terminal) return;
				terminal = true;
				cleanup();
				try { this.xhr.abort(); } catch { /* No pending network operation. */ }
				reject(new HTTPError(0, sentMutation
					? `${reason} The operation may have succeeded. Reload and check the current state before submitting it again.`
					: reason, sentMutation ? "Request outcome unknown" : "Request failed"));
			};
			const succeed = (response: HTTPResponse<T>) => {
				if (terminal) return;
				terminal = true; cleanup(); resolve(response);
			};
			const schedule = () => {
				if (terminal || this.cancelled) return;
				window.clearTimeout(timer);
				timer = window.setTimeout(attempt, 1000);
			};
			const onTransportFailure = () => {
				if (terminal) return;
				if (this.method === "GET" && !this.cancelled && attempts < this.maxAttempts) schedule();
				else fail("The connection ended before a reliable response was received.");
			};
			const onAbort = () => fail("The request was cancelled.");
			const onLoad = () => {
				if (terminal || this.cancelled) return;
				if (!this.xhr.status) return onTransportFailure();
				if (this.method === "GET" && this.xhr.status === StatusCodes.TOO_MANY_REQUESTS && attempts < this.maxAttempts) return schedule();
				let body: T | undefined;
				const contentType = this.xhr.getResponseHeader("Content-Type") ?? "";
				if (this.xhr.response) {
					if (contentType.includes("text/plain")) body = this.xhr.responseText as T;
					else if (contentType.includes("application/json")) {
						try { body = JSON.parse(this.xhr.response); } catch { body = this.xhr.response as T; }
					}
				}
				succeed({ status: this.xhr.status, body, headers: {
					"x-weebdex-instance-removal": this.xhr.getResponseHeader("x-weebdex-instance-removal"),
					"x-weebdex-source-deletion-status": this.xhr.getResponseHeader("x-weebdex-source-deletion-status"),
					"x-weebdex-source-deletion-request": this.xhr.getResponseHeader("x-weebdex-source-deletion-request")
				} });
			};
			const attempt = () => {
				if (terminal || this.cancelled) return;
				// Poll only our cancellable timer. Never queue an uncancellable limiter promise.
				if (!limiter.tryRemoveTokens(1)) {
					timer = window.setTimeout(attempt, 250);
					return;
				}
				try {
					const payload = this.body === undefined ? undefined : this.body instanceof FormData ? this.body : JSON.stringify(this.body);
					this.open();
					if (terminal || this.cancelled) return;
					attempts++;
					if (payload !== undefined && !(payload instanceof FormData)) this.xhr.setRequestHeader("Content-Type", "application/json");
					sentMutation ||= this.method !== "GET";
					this.xhr.send(payload);
				} catch {
					fail("The request could not be completed.");
				}
			};
			this.cancelPending = () => fail("The request was cancelled.");
			this.deadlinePending = () => fail("The request timed out.");
			this.xhr.addEventListener("load", onLoad);
			this.xhr.addEventListener("error", onTransportFailure);
			this.xhr.addEventListener("timeout", onTransportFailure);
			this.xhr.addEventListener("abort", onAbort);
			if (this.cancelled) return fail("The request was cancelled before it was sent.");
			this.tick(this.timeoutMs);
			attempt();
		});
	}
	GET() { if (this.isAlreadyOpened) return; this.method = "GET"; return this.send(); }
	POST() { if (this.isAlreadyOpened) return; this.method = "POST"; return this.send(); }
	PATCH() { if (this.isAlreadyOpened) return; this.method = "PATCH"; return this.send(); }
	PUT() { if (this.isAlreadyOpened) return; this.method = "PUT"; return this.send(); }
	DELETE() { if (this.isAlreadyOpened) return; this.method = "DELETE"; return this.send(); }
	setWithCredentials(value: boolean) { this.xhr.withCredentials = value; }
	setRequestHeader(name: string, value: string) { this.headers[name] = value; }
	setMaxAttempts(attempts: number) { this.maxAttempts = Number.isFinite(attempts) ? Math.max(1, Math.min(10, Math.floor(attempts))) : 3; }
	addEventListener<K extends keyof XMLHttpRequestEventTargetEventMap>(key: K, listener: (event: XMLHttpRequestEventTargetEventMap[K]) => void) { this.xhr.addEventListener(key, listener); }
	addUploadListener<K extends keyof XMLHttpRequestEventTargetEventMap>(key: K, listener: (event: XMLHttpRequestEventTargetEventMap[K]) => void) { this.xhr.upload.addEventListener(key, listener); }
	cancel() { this.cancelled = true; this.cancelPending?.(); }
	tick(timeout = 60000) {
		if (this.finished || this.cancelled) return;
		this.timeoutMs = Number.isFinite(timeout) ? Math.max(1, Math.min(300000, timeout)) : 60000;
		window.clearTimeout(this.timeout);
		if (this.deadlinePending) this.timeout = window.setTimeout(() => this.deadlinePending?.(), this.timeoutMs);
	}
}
export default HTTP;
