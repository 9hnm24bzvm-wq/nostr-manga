import type { Attachment } from "svelte/attachments";

export const exec = (element: Element) => {
	for (const image of element.querySelectorAll("img[data-image]")) {
		if (image instanceof HTMLImageElement) {
			const src = image.getAttribute("src");
			if (src) {
				image.addEventListener("load", () => {
					image.width = image.naturalWidth;
					image.height = image.naturalHeight;
				});
			}
		}
	}
	for (const spoiler of element.querySelectorAll("span[data-spoiler]")) {
		if (!spoiler.hasAttribute("data-clickable")) {
			spoiler.addEventListener("click", () => {
				spoiler.toggleAttribute("data-visible");
			});
			spoiler.setAttribute("data-clickable", "");
		}
	}
	for (const quote of element.querySelectorAll("[data-quote]")) {
		if (quote.hasAttribute("data-quote-post") && !quote.hasAttribute("data-linked")) {
			const header = quote.querySelector("[data-quote-header]");
			if (!header) {
				continue;
			}

			const attribute = header.querySelector("[data-quote-attribute]");
			if (!attribute) {
				continue;
			}

			const target = document.getElementById(`p${quote.getAttribute("data-quote-post")}`);
			const anchor = document.createElement("a");
			const threadId = document.getElementById("comments")?.getAttribute("data-thread-id");

			if (target) {
				anchor.href = `#p${quote.getAttribute("data-quote-post")}`;
				anchor.addEventListener("click", e => {
					e.preventDefault();
					target.scrollIntoView({ block: "center" });
					target.setAttribute("data-highlight", "true");

					window.setTimeout(() => {
						target.removeAttribute("data-highlight");
					}, 2000);
				});
			} else if (threadId) {
				anchor.href = `/forums/threads/${threadId}/${quote.getAttribute("data-quote-post")}`;
				anchor.rel = "nofollow";
			}

			header.replaceChild(anchor, attribute);
			anchor.appendChild(attribute);
		}
	}
};

export const observer: Attachment = (element: Element) => {
	const observer = new MutationObserver(() => exec(element));
	window.requestAnimationFrame(() => {
		observer.observe(element, {
			childList: true,
			subtree: true
		});
		exec(element);
	});
	return () => {
		observer.disconnect();
	};
};
