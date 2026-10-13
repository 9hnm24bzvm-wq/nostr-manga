import { clsx, type ClassValue } from "clsx";
import DOMPurify from "dompurify";
import { isbot } from "isbot";
import { marked } from "marked";
import slugify from "slugify";
import { twMerge } from "tailwind-merge";

export const isBot = (): boolean => {
	return window.navigator.webdriver || isbot(window.navigator.userAgent);
};

export const isValidURL = (value: string) => {
	try {
		new URL(value);
		return true;
	} catch {
		return false;
	}
};

export const makeSlug = (value: string) => {
	let result = slugify(value, { lower: true, strict: true, trim: true, locale: "en" });
	if (result.length) {
		result = "/" + result;
	}
	return result;
};

export const markdownPreprocessor = (md: string) => {
	const result: string[] = [];
	for (const line of md.split("\n")) {
		if (line.trim()) result.push(line);
		else result.push("<!-- Break -->");
	}
	return result.join("\n");
};

export const parseDescription = (value?: string) => {
	if (!value) return "";

	const unsafe = marked.parse(value, { gfm: true, breaks: true }) as string;
	return DOMPurify.sanitize(unsafe, {
		ALLOWED_TAGS: [
			"span",
			"a",
			"b",
			"strong",
			"i",
			"em",
			"u",
			"s",
			"del",
			"p",
			"ul",
			"ol",
			"li",
			"pre",
			"code",
			"blockquote",
			"table",
			"thead",
			"tbody",
			"details",
			"summary",
			"tr",
			"th",
			"td",
			"br",
			"hr",
			"img"
		],
		ALLOWED_ATTR: ["href"]
	});
};

export const parseDescriptionPlain = (value?: string) => {
	if (!value) return "";

	const unsafe = marked.parse(value, { gfm: true, breaks: true }) as string;
	return DOMPurify.sanitize(unsafe, {
		ALLOWED_TAGS: [
			"span",
			"a",
			"b",
			"strong",
			"i",
			"em",
			"u",
			"s",
			"del",
			"p",
			"ul",
			"ol",
			"li",
			"pre",
			"code",
			"details",
			"summary",
			"tr",
			"th",
			"td",
			"br",
			"hr"
		],
		ALLOWED_ATTR: ["href"]
	});
};

export const parseComment = (value?: string) => {
	if (!value) return;

	const unsafe = marked.parse(markdownPreprocessor(value), { gfm: true, breaks: true }) as string;
	return DOMPurify.sanitize(unsafe, {
		ALLOWED_TAGS: [
			"h1",
			"h2",
			"h3",
			"h4",
			"h5",
			"div",
			"span",
			"a",
			"b",
			"strong",
			"i",
			"em",
			"u",
			"s",
			"del",
			"p",
			"ul",
			"ol",
			"li",
			"blockquote",
			"details",
			"summary",
			"br",
			"img"
		],
		ALLOWED_ATTR: ["href"]
	});
};

export const parseQuote = (value?: string) => {
	if (!value) return;

	const unsafe = marked.parse(markdownPreprocessor(value), { gfm: true, breaks: true }) as string;
	return DOMPurify.sanitize(unsafe, {
		ALLOWED_TAGS: [
			"h1",
			"h2",
			"h3",
			"h4",
			"h5",
			"span",
			"a",
			"b",
			"strong",
			"i",
			"em",
			"u",
			"s",
			"del",
			"p",
			"ul",
			"ol",
			"li",
			"details",
			"summary",
			"br",
			"img"
		],
		ALLOWED_ATTR: ["href"]
	});
};

export const groupTags = (value: Tag[]) => {
	return value.reduce<Record<string, Tag[]>>(
		(acc, tag) => {
			(acc[tag.group] ??= []).push(tag);
			return acc;
		},
		{} as Record<string, Tag[]>
	);
};

export const chapterHeadline = (chapter: Chapter | ChapterAggregate | ReaderChapterInfo, full = true) => {
	let result = "";

	if (full && chapter.volume) {
		result = `Vol. ${chapter.volume} `;
	}
	if (chapter.chapter) {
		if (full) {
			result += "Ch. ";
		} else {
			result += "Chapter ";
		}
		result += chapter.chapter;
	} else {
		result += "Oneshot";
	}
	if (
		full &&
		(<Chapter>chapter).title &&
		(<Chapter>chapter).title!.localeCompare("oneshot", undefined, { sensitivity: "accent" }) !== 0
	) {
		result += ` - ${(<Chapter>chapter).title}`;
	}
	return result;
};

export const chapterHeadlineWithoutVolume = (chapter: Chapter | ChapterAggregate | ReaderChapterInfo) => {
	let result = "";

	if (chapter.chapter) {
		result += "Ch. ";
		result += chapter.chapter;
	} else {
		result += "Oneshot";
	}
	if (
		(<Chapter>chapter).title &&
		(<Chapter>chapter).title!.localeCompare("oneshot", undefined, { sensitivity: "accent" }) !== 0
	) {
		result += ` - ${(<Chapter>chapter).title}`;
	}
	return result;
};

const volumeChapterRgx = /^((0|[1-9]\d*)(\.\d+)?[a-z]?)?$/;
export const isValidVolumeChapter = (value: string): boolean => {
	return !!value && volumeChapterRgx.test(value);
};

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
