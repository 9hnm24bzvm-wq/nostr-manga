import { PUBLIC_PROXY_URL } from "$env/static/public";
import * as MarkedExtensions from "$lib/MarkedExtensions";
import { formatDistance } from "date-fns";
import DOMPurify from "dompurify";
import { marked, type Tokens } from "marked";

export const prerender = false;
export const ssr = false;

const _toString = URLSearchParams.prototype.toString;
URLSearchParams.prototype.toString = function (this: URLSearchParams) {
	let result = _toString.call(this);
	if (result && !result.startsWith("?")) {
		result = "?" + result;
	}
	return result;
};

Number.prototype.short = function (this: number, digits = 1) {
	// eslint-disable-next-line @typescript-eslint/no-this-alias
	const self = this;
	const sign = self < 0 ? "-" : "";
	const abs = Math.abs(self);

	const lookup = [
		{ value: 1, symbol: "" },
		{ value: 1e3, symbol: "K" },
		{ value: 1e6, symbol: "M" },
		{ value: 1e9, symbol: "G" },
		{ value: 1e12, symbol: "T" },
		{ value: 1e15, symbol: "P" },
		{ value: 1e18, symbol: "E" }
	];

	const rx = /\.0+$|(\.[0-9]*[1-9])0+$/;
	const item = lookup
		.slice()
		.reverse()
		.find(item => abs >= item.value);

	return item ? sign + (abs / item.value).toFixed(digits).replace(rx, "$1") + item.symbol : "0";
};

String.prototype.short = function (this: string) {
	const seconds = Math.floor((Date.now() - +new Date(this)) / 1000);

	const intervals = [
		{ label: "y", seconds: 31536000 },
		{ label: "mo", seconds: 2592000 },
		{ label: "w", seconds: 604800 },
		{ label: "d", seconds: 86400 },
		{ label: "h", seconds: 3600 },
		{ label: "m", seconds: 60 }
	];

	for (const { label, seconds: s } of intervals) {
		const count = Math.floor(seconds / s);
		if (count >= 1) return `${count}${label}`;
	}

	return `${seconds}s`;
};

String.prototype.ago = function (this: string) {
	const result = formatDistance(new Date(this), new Date(), { addSuffix: true });
	return result.replaceAll("about ", "").replaceAll("less than a minute ago", "just now");
};

const isSafeUrl = (url: string) => !/^\s*(?:javascript|data):/i.test(url);
DOMPurify.addHook("afterSanitizeAttributes", node => {
	switch (node.tagName) {
		case "A": {
			const href = node.getAttribute("href") || "";
			node.setAttribute("rel", "nofollow noopener noreferrer");

			if (!isSafeUrl(href)) node.removeAttribute("href");
			if (!href.startsWith("#p")) node.setAttribute("target", "_blank");

			break;
		}
		case "IMG": {
			const src = node.getAttribute("data-src") || "";
			node.removeAttribute("data-src");

			if (isSafeUrl(src)) {
				if (src.includes("://i.imgur.com")) {
					node.setAttribute("src", src);
				} else {
					node.setAttribute("src", `${PUBLIC_PROXY_URL}?image=${encodeURIComponent(src)}`);
				}
			}

			node.setAttribute("loading", "lazy");
			node.setAttribute("referrerpolicy", "no-referrer");
			break;
		}
	}
});

const renderer = new marked.Renderer();
renderer.image = function (token: Tokens.Image) {
	return `<img data-src="${token.href}" data-image />`;
};

renderer.html = function (token: Tokens.HTML) {
	if (token.text.includes("<!-- Break -->")) {
		return `<br />`;
	}
	return "";
};

marked.use({
	renderer,
	extensions: [
		MarkedExtensions.underlineExtension,
		MarkedExtensions.attributedQuoteExtension,
		MarkedExtensions.greentextExtension,
		MarkedExtensions.quotelinkExtension,
		MarkedExtensions.spoilerExtension,
		MarkedExtensions.collapsibleExtension
	]
});
