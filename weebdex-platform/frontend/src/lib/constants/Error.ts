export class Error {
	constructor(
		readonly title: string,
		readonly body: string
	) {}
}

export const TooManyRequests = new Error(
	"Too Many Requests",
	"You've made too many requests. Please wait a moment and try again."
);

export const Forbidden = new Error("Forbidden", "You do not have permission to perform this action.");

export const UpdateConflict = new Error(
	"Version Mismatch",
	"Your changes were not saved due to a conflict. Please refresh and try again."
);

export const Unknown: Error = new Error("Unknown Error", "Something went wrong");
