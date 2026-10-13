export const rules: Record<string, ExternalLinkRule> = {
	twitter: {
		regex: /^https?:\/\/(?:[\w-]+\.)?(?:twitter|x)\.com(\/|$)/,
		full: true
	},
	pixiv: {
		regex: /^https?:\/\/(?:[\w-]+\.)?pixiv\.net(\/|$)/,
		full: true
	},
	fanbox: {
		regex: /^https?:\/\/(?:[\w-]+\.)?fanbox\.cc(\/|$)/,
		full: true
	},
	fantia: {
		regex: /^https?:\/\/(?:[\w-]+\.)?fantia\.jp(\/|$)/,
		full: true
	},
	skeb: {
		regex: /^https?:\/\/(?:[\w-]+\.)?skeb\.jp(\/|$)/,
		full: true
	},
	youtube: {
		regex: /^https?:\/\/(?:[\w-]+\.)?youtube\.com(\/|$)/,
		full: true
	},
	website: { full: true }
};

export const baseUrls: Record<string, string> = {
	twitter: "https://twitter.com/",
	pixiv: "https://www.pixiv.net/",
	fanbox: "https://www.fanbox.cc/",
	fantia: "https://fantia.jp/",
	skeb: "https://skeb.jp/",
	youtube: "https://www.youtube.com/"
};

export const names: Record<string, string> = {
	twitter: "Twitter",
	pixiv: "Pixiv",
	fanbox: "FANBOX",
	fantia: "Fantia",
	skeb: "Skeb",
	youtube: "YouTube",
	website: "Website"
};

export const placeholders: Record<string, string> = {
	twitter: "e.g., https://twitter.com/example",
	pixiv: "e.g., https://www.pixiv.net/users/12345",
	fanbox: "e.g., https://example.fanbox.cc",
	fantia: "e.g., https://fantia.jp/fanclubs/12345",
	skeb: "e.g., https://skeb.jp/@example",
	youtube: "e.g., https://www.youtube.com/@example",
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
