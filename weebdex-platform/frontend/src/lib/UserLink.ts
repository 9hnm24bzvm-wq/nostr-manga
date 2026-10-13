export const rules: Record<string, ExternalLinkRule> = {
	discord: {},
	twitter: {
		regex: /^https?:\/\/(?:[\w-]+\.)?(twitter|x)\.com(\/|$)/,
		full: true
	},
	website: { full: true }
};

export const baseUrls: Record<string, string> = {
	discord: "https://discord.gg/",
	twitter: "https://twitter.com/"
};

export const names: Record<string, string> = {
	discord: "Discord",
	twitter: "Twitter",
	website: "Website"
};

export const placeholders: Record<string, string> = {
	discord: "e.g., https://discord.gg/code or code or /invite/code",
	twitter: "e.g., https://twitter.com/example",
	website: "e.g., https://weebdex.org"
};

export const build = (code: string, link: string): string => {
	if (link.startsWith("http") || !(code in baseUrls)) {
		return link;
	}

	if (link.startsWith("/")) {
		link = link.slice(1);
	}
	return baseUrls[code] + link;
};

export const validate = (code: string, link: string): string => {
	code = code.trim();
	link = link.trim();

	if (!(code in rules)) {
		return "";
	}

	const rule = rules[code];
	if (link.startsWith("http")) {
		if (rule.regex) {
			const matches = link.match(rule.regex);
			if (!matches?.length) return "";
			if (rule.extract) {
				link = rule.extract(matches);
			}
		}
	} else if (rule.full) {
		return "";
	}
	return link;
};

export const getCodeFromUrl = (url: string): string | undefined => {
	for (const [code, rule] of Object.entries(rules)) {
		if (rule.regex && rule.regex.test(url)) {
			return code;
		}
	}
	return undefined;
};
