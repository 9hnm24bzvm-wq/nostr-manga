export enum Status {
	Refused = "refused",
	Waiting = "waiting",
	Accepted = "accepted",
	Autoresolved = "autoresolved"
}

export const StatusText: Record<string, string> = {
	[Status.Refused]: "Refused",
	[Status.Waiting]: "Pending review",
	[Status.Accepted]: "Accepted",
	[Status.Autoresolved]: "Auto-resolved"
};
