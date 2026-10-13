export const rules: Record<string, ExternalLinkRule> = {
	al: {
		regex: /^https?:\/\/(?:[\w-]+\.)?anilist\.co\/manga\/([0-9]+)/,
		extract: (s: string[]) => s[1]
	},
	ap: {
		regex: /^https?:\/\/(?:[\w-]+\.)?anime-planet\.com\/manga\/([a-zA-Z0-9-]+)/,
		extract: (s: string[]) => s[1]
	},
	mu: {
		regex: /^https?:\/\/(?:[\w-]+\.)?mangaupdates\.com\/(?:series\/([a-zA-Z0-9-]+)|series\.html\?id=([a-zA-Z0-9-]+))/,
		extract: (m: string[]) => (m[1] ?? "") + (m[2] ?? "")
	},
	nu: {
		regex: /^https?:\/\/(?:[\w-]+\.)?novelupdates\.com\/series\/([a-zA-Z0-9-]+)/,
		extract: (s: string[]) => s[1]
	},
	kt: {
		regex: /^https?:\/\/(?:[\w-]+\.)?kitsu\.(?:io|app)\/manga\/([a-zA-Z0-9-]+)/,
		extract: (s: string[]) => s[1]
	},
	mal: {
		regex: /^https?:\/\/(?:[\w-]+\.)?myanimelist\.net\/manga\/([0-9]+)/,
		extract: (s: string[]) => s[1]
	},

	bw: {
		regex: /^https?:\/\/(?:[\w-]+\.)?bookwalker\.jp\/(.+)/,
		extract: (s: string[]) => s[1]
	},
	amz: {
		regex: /^https?:\/\/(?:[\w-]+\.)?amazon\.(?:com|co\.jp)\//,
		full: true
	},
	ebj: {
		regex: /^https?:\/\/ebookjapan\.yahoo\.co\.jp\//,
		full: true
	},
	cdj: {
		regex: /^https?:\/\/(?:[\w-]+\.)?cdjapan\.co\.jp\//,
		full: true
	},
	raw: {
		full: true
	},
	engtl: {
		full: true
	}
};

export const baseUrls: Record<string, string> = {
	al: "https://anilist.co/manga/",
	ap: "https://www.anime-planet.com/manga/",
	mu: "https://www.mangaupdates.com/series/",
	mu_n: "https://www.mangaupdates.com/series.html?id=",
	nu: "https://www.novelupdates.com/series/",
	kt: "https://kitsu.app/manga/",
	mal: "https://myanimelist.net/manga/",

	bw: "https://bookwalker.jp/",
	amz: "https://www.amazon.co.jp/",
	ebj: "https://ebookjapan.yahoo.co.jp/",
	cdj: "https://www.cdjapan.co.jp/"
};

export const groups: Record<string, string[]> = {
	1: ["al", "ap", "mu", "nu", "kt", "mal"],
	2: ["bw", "amz", "ebj", "cdj", "raw", "engtl"]
};

export const names: Record<string, string> = {
	al: "AniList",
	ap: "Anime-Planet",
	mu: "MangaUpdates",
	nu: "Novel Updates",
	kt: "Kitsu",
	mal: "MyAnimeList",

	bw: "Book Walker",
	amz: "Amazon",
	ebj: "eBookJapan",
	cdj: "CDJapan",
	raw: "Official Raw",
	engtl: "Official English"
};

export const placeholders: Record<string, string> = {
	al: "e.g., https://anilist.co/manga/123456",
	ap: "e.g., https://anime-planet.com/manga/example",
	mu: "e.g., https://www.mangaupdates.com/series/example",
	nu: "e.g., https://www.novelupdates.com/series/example",
	kt: "e.g., https://kitsu.app/manga/example",
	mal: "e.g., https://myanimelist.net/manga/123456",

	bw: "e.g., https://bookwalker.jp/example",
	amz: "e.g., https://www.amazon.co.jp/",
	ebj: "e.g., https://ebookjapan.yahoo.co.jp/",
	cdj: "e.g., https://www.cdjapan.co.jp/"
};

export const group = (links: Record<string, string>) => {
	const result: Record<string, Record<string, string>> = {};
	for (const [code, link] of Object.entries(links)) {
		if (groups[1].includes(code)) {
			(result[1] ??= {})[code] = link;
		} else if (groups[2].includes(code)) {
			(result[2] ??= {})[code] = link;
		}
	}
	return result;
};

export const build = (code: string, link: string): string => {
	if (code === "mu" && Number.isInteger(Number(link))) {
		code += "_n";
	}

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

export const base = (str: string) => {
	const url = new URL(str);
	return `${url.protocol}//${url.host}`;
};

export const getCodeFromUrl = (url: string): string | undefined => {
	for (const [code, rule] of Object.entries(rules)) {
		if (rule.regex && rule.regex.test(url)) {
			return code;
		}
	}
	return undefined;
};
