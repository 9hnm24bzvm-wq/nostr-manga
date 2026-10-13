import { Utils } from "$lib";
import { type RendererExtension, type TokenizerExtension, type Tokens, marked } from "marked";

export const underlineExtension: TokenizerExtension & RendererExtension = {
	name: "underline",
	level: "inline",

	start(src: string) {
		return src.match(/__/)?.index;
	},

	tokenizer(src: string) {
		const rule = /^__([^_]+?)__/;
		const match = rule.exec(src);
		if (match) {
			return {
				type: "underline",
				raw: match[0],
				text: match[1]
			};
		}
	},

	renderer(token: Tokens.Generic) {
		return `<u data-underline>${token.text}</u>`;
	}
};

export const attributedQuoteExtension: TokenizerExtension & RendererExtension = {
	name: "attributed-quote",
	level: "block",

	start(src) {
		return src.match(/^>\s*\[[^\]]+\]/m)?.index;
	},

	tokenizer(src) {
		const rule = /^(>\s*\[[^\]]+\][^\n]*(?:\n>\s*[^\n]*)*)/;
		const match = rule.exec(src);

		if (!match) return;

		const raw = match[0];
		const lines = raw.split("\n");

		// Extract attribute block from first line
		const attrMatch = lines[0].match(/>\s*\[([^\]]+)\]\s*(.*)/);
		if (!attrMatch) return;

		const attrs = Object.fromEntries(
			attrMatch[1].split(",").map(pair => {
				const [k, v] = pair.split("=");
				return [k.trim(), v?.trim()];
			})
		);

		lines[0] = attrMatch[2]; // remove attribute part

		return {
			type: "attributed-quote",
			raw,
			attrs,
			text: lines.join("\n").replace(/^>\s?/gm, "")
		};
	},

	renderer(token: Tokens.Generic) {
		if (token.attrs.quote && token.attrs.user && token.attrs.post) {
			return `
			<div data-quote data-quote-post="${token.attrs.post}">
				<div data-quote-header>
					<strong data-quote-attribute>${token.attrs.quote} said:</strong>
				</div>
				<div data-quote-content>
					<blockquote>
					${Utils.parseQuote(token.text)}
					</blockquote>
				</div>
			</div>`;
		}
		return `
			<blockquote>
				${Utils.parseQuote(token.text)}
			</blockquote>`;
	}
};

export const greentextExtension: TokenizerExtension & RendererExtension = {
	name: "greentext",
	level: "block",

	start(src: string) {
		return src.match(/^>(?!\s)/m)?.index;
	},

	tokenizer(src: string) {
		const rule = /^(>(?!\s)[^\n]*(?:\n>(?!\s)[^\n]*)*)/;
		const match = rule.exec(src);

		if (match) {
			return {
				type: "greentext",
				raw: match[0],
				text: match[0]
			};
		}
	},

	renderer(token: Tokens.Generic) {
		return (token.text as string)
			.split("\n")
			.map(line => `<blockquote data-greentext><p>${marked.parseInline(line)}</p></blockquote>`)
			.join("");
	}
};

export const quotelinkExtension: TokenizerExtension & RendererExtension = {
	name: "greentext-link",
	level: "inline",

	start(src: string) {
		return src.match(/>>/)?.index;
	},

	tokenizer(src: string) {
		const rule = /^(>>\d+)/;
		const match = rule.exec(src);
		if (match) {
			return {
				type: "greentext-link",
				raw: match[0],
				text: match[1]
			};
		}
	},

	renderer(token: Tokens.Generic) {
		const id = token.text.slice(2);
		return `<a href="#p${id}" data-greentext-link>${token.text}</a>`;
	}
};

export const spoilerExtension: TokenizerExtension & RendererExtension = {
	name: "spoiler",
	level: "inline",

	start(src: string) {
		return src.match(/\|\|/)?.index;
	},

	tokenizer(src: string) {
		const rule = /^\|\|([\s\S]+?)\|\|(?!\|)/;
		const match = rule.exec(src);
		if (match) {
			return {
				type: "spoiler",
				raw: match[0],
				tokens: this.lexer.inlineTokens(match[1])
			};
		}
	},

	renderer(token: Tokens.Generic) {
		return token.tokens?.length ? `<span data-spoiler>${this.parser.parseInline(token.tokens)}</span>` : token.raw;
	}
};

export const collapsibleExtension: TokenizerExtension & RendererExtension = {
	name: "collapsible",
	level: "block",

	start(src: string) {
		return src.match(/\+\+\+/)?.index;
	},

	tokenizer(src: string) {
		const rule = /^\+\+\+\s*(.*?)\n([\s\S]*?)\n\+\+\+/;
		const match = rule.exec(src);
		if (match) {
			return {
				type: "collapsible",
				raw: match[0],
				title: match[1].trim(),
				text: match[2].trim(),
				tokens: this.lexer.blockTokens(match[2].trim())
			};
		}
	},

	renderer(token: Tokens.Generic) {
		return token.tokens?.length
			? `<details data-collapsible><summary>${token.title}</summary>\n${this.parser.parse(token.tokens)}</details>`
			: token.raw;
	}
};
