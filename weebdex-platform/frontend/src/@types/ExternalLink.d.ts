declare global {
	interface ExternalLink {
		code: string;
		link: string;
		valid: boolean;
	}
	interface ExternalLinkRule {
		regex?: RegExp;
		extract?: (s: string[]) => string;
		full?: boolean;
	}
}
export {};
