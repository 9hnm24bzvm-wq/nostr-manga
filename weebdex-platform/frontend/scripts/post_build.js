import fs from "fs";
import path from "path";

const dir = "./build";

function minifyHTML(html) {
	return (
		html
			// Remove HTML comments
			.replace(/<!--[\s\S]*?-->/g, "")
			// Remove leading and trailing whitespace between tags
			.replace(/\s*(<[^>]+>)\s*/g, "$1")
			// Collapse multiple spaces into one
			.replace(/\s{2,}/g, " ")
			// Remove whitespace between tags (optional, may break pre/code)
			.replace(/>\s+</g, "><")
			.trim()
	);
}

fs.readdirSync(dir).forEach(file => {
	if (file.endsWith("index.html")) {
		const filePath = path.join(dir, file);
		let base = fs.readFileSync(filePath, "utf-8");
		base = base.replace("<!-- GO_TEMPLATE_END -->", "{{- end -}}");

		for (const [key, value] of Object.entries({
			index: "index_meta.html",
			manga: "manga_meta.html",
			chapter: "chapter_meta.html",
			group: "group_meta.html"
		})) {
			const meta = fs.readFileSync(path.join("./scripts", value), "utf-8");
			fs.writeFileSync(
				path.join(dir, "templates", `${key}.html`),
				minifyHTML(
					base
						.replace("<!-- GO_TEMPLATE_START -->", `{{- define "${key}" -}}`)
						.replace("<!-- GO_TEMPLATE_META -->", meta)
				)
			);
		}
		// remove file
		fs.unlinkSync(filePath);
	}
});
