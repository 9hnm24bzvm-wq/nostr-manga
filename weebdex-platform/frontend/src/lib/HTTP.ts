import { StatusCodes } from "http-status-codes";
import { RateLimiter } from "limiter";

export class HTTPError {
	constructor(
		public code: number,
		public error: string
	) {}
}

export interface HTTPResponse<T = void> {
	status: number;
	body?: T;
}

const limiter = new RateLimiter({ tokensPerInterval: 5, interval: "second" });

class HTTP<T, U = unknown> {
	private xhr: XMLHttpRequest;

	private timeout = 0;
	private maxAttempts = 3;
	private isAlreadyOpened = false;

	private method: "GET" | "POST" | "PATCH" | "PUT" | "DELETE" = "GET";
	private headers: Record<string, string> = {};

	constructor(
		private readonly url: string,
		private readonly body?: U
	) {
		this.xhr = new XMLHttpRequest();
	}

	private open() {
		this.xhr.open(this.method, this.url);
		for (const [k, value] of Object.entries(this.headers)) {
			this.xhr.setRequestHeader(k, value);
		}
	}

	private send(): Promise<HTTPResponse<T>> {
		let attempts = 0;
		return new Promise((resolve, reject) => {
			const tryAttempt = () =>
				limiter.removeTokens(1).then(() => {
					this.open();
					attempts++;

					if (this.body) {
						if (this.body instanceof FormData) {
							return this.xhr.send(this.body);
						}

						this.xhr.setRequestHeader("Content-Type", "application/json");
						return this.xhr.send(JSON.stringify(this.body));
					}
					this.xhr.send();
				});

			this.xhr.addEventListener("load", () => {
				window.clearTimeout(this.timeout);
				if (this.xhr.status === StatusCodes.TOO_MANY_REQUESTS) {
					if (attempts > this.maxAttempts) {
						resolve({ status: this.xhr.status });
					} else {
						window.setTimeout(() => tryAttempt(), 1000);
					}
				} else {
					const status = this.xhr.status;
					let response: T | undefined;

					if (this.xhr.response) {
						const ct = this.xhr.getResponseHeader("Content-Type");
						if (ct?.includes("text/plain")) {
							response = this.xhr.responseText as T;
						} else if (ct?.includes("application/json")) {
							try {
								response = JSON.parse(this.xhr.response);
							} catch {
								response = this.xhr.response;
							}
						} else {
							console.warn("[HTTP] unhandled response content-type", ct);
						}
					}
					resolve({ status, body: response });
				}
			});

			this.xhr.addEventListener("abort", () => {
				window.clearTimeout(this.timeout);
				reject(new HTTPError(this.xhr.status, "request has been aborted"));
			});

			this.xhr.addEventListener("error", () => {
				window.clearTimeout(this.timeout);
				if (attempts > this.maxAttempts) {
					resolve({ status: this.xhr.status });
				} else {
					window.setTimeout(() => tryAttempt(), 1000);
				}
			});
			tryAttempt();
		});
	}

	GET() {
		if (this.isAlreadyOpened) {
			return;
		}
		this.method = "GET";
		return this.send();
	}

	POST() {
		if (this.isAlreadyOpened) {
			return;
		}
		this.method = "POST";
		return this.send();
	}

	PATCH() {
		if (this.isAlreadyOpened) {
			return;
		}
		this.method = "PATCH";
		return this.send();
	}

	PUT() {
		if (this.isAlreadyOpened) {
			return;
		}
		this.method = "PUT";
		return this.send();
	}

	DELETE() {
		if (this.isAlreadyOpened) {
			return;
		}
		this.method = "DELETE";
		return this.send();
	}

	setWithCredentials(value: boolean) {
		this.xhr.withCredentials = value;
	}

	setRequestHeader(name: string, value: string): void {
		this.headers[name] = value;
	}

	setMaxAttempts(attempts: number) {
		this.maxAttempts = attempts;
	}

	addEventListener<K extends keyof XMLHttpRequestEventTargetEventMap>(
		k: K,
		listener: (ev: XMLHttpRequestEventTargetEventMap[K]) => void
	) {
		this.xhr.addEventListener(k, listener);
	}

	addUploadListener<K extends keyof XMLHttpRequestEventTargetEventMap>(
		k: K,
		listener: (ev: XMLHttpRequestEventTargetEventMap[K]) => void
	) {
		this.xhr.upload.addEventListener(k, listener);
	}

	cancel() {
		this.xhr.abort();
	}

	tick(timeout = 1000 * 60) {
		window.clearTimeout(this.timeout);
		this.timeout = window.setTimeout(() => this.xhr.abort(), timeout);
	}
}

export default HTTP;
