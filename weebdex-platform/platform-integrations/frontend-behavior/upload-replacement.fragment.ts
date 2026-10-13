	const replaceFile = async (file: File) => {
		if (!targetFile || !uploadSession || uploadSession.can_mutate === false || disabled) return;
		const target = targetFile, idx = pages.indexOf(target); if (idx < 0) return;
		if (!imageMimeTypes.includes(file.type) || !file.size || file.size > maxFileSize) throw new Constants.Error.Error(file.name, "Replacement must be a valid image within the 25 MiB page limit. Existing page is retained.");
		const header = new Uint8Array(await file.slice(0, 12).arrayBuffer()), extension = file.name.toLowerCase().match(/\.(png|gif|jpe?g)$/)?.[1];
		const actual = header.length >= 8 && [137,80,78,71,13,10,26,10].every((byte,index) => header[index] === byte) ? "png" : header.length >= 6 && ["GIF87a","GIF89a"].includes(String.fromCharCode(...header.subarray(0,6))) ? "gif" : header.length >= 3 && header[0] === 255 && header[1] === 216 && header[2] === 255 ? "jpeg" : undefined;
		if (!actual || actual !== (extension === "jpg" ? "jpeg" : extension) || (file.type.replace("image/jpg", "image/jpeg") !== `image/${actual}`)) throw new Constants.Error.Error(file.name, "Image filename, MIME and encoded format must agree. Existing page is retained.");
		const decoded = await decodeUploadImage(file);
		if (decoded.width > maxImageWidth || decoded.height > maxImageHeight) throw new Constants.Error.Error(file.name, "A replacement must fit one bounded page. Add this image normally to split it; existing page is retained.");
		const total = pages.reduce((sum, page) => sum + Number(page.file_size ?? page.file?.size ?? 0), 0), previousSize = Number(target.file_size ?? target.file?.size ?? 0);
		if (!Number.isFinite(total) || !Number.isFinite(previousSize) || pages.length > maxSessionFiles || total - previousSize + file.size > maxSessionSize) throw new Constants.Error.Error(file.name, "Replacement exceeds the 500-page/500 MiB session budget. Existing page is retained.");
		// A local rejected entry has no durable old file to replace.
		if (target.file || target.error) {
			const candidate = createPage(file); pages[idx] = candidate;
			try { await uploadChunk([candidate]); revokeThumbnail(target); }
			catch (error) { pages[idx] = target; revokeThumbnail(candidate); throw error; }
			return;
		}
		const current = await API.UploadSession.Get();
		if (current?.status !== StatusCodes.OK || current.body?.id !== uploadSession.id || current.body.can_mutate === false) throw new Constants.Error.Error("Replacement unavailable", "Current owner staging authority was not confirmed. Existing page is retained.");
		const old = current.body.relationships.files?.find(row => row.id === target.id);
		if (!old || target.file_hash && old.file_hash !== target.file_hash) throw new Constants.Error.Error("Page changed", "This staged page changed in another tab. Reload before replacing it.");
		const sha = [...new Uint8Array(await crypto.subtle.digest("SHA-256", await file.arrayBuffer()))].map(byte => byte.toString(16).padStart(2,"0")).join("");
		const form = new FormData(); form.append("file", file);
		const http = new HTTP<UploadSessionFile & { version: number }>(`${PUBLIC_API_URL}/upload/${uploadSession.id}/files/${target.id}/replace`, form);
		http.setWithCredentials(true); http.setRequestHeader("X-WeebDex-Upload-Version", String(current.body.version));
		let replacement: UploadSessionFile | undefined, version = current.body.version;
		try {
			const result = await http.POST();
			if (result?.status !== StatusCodes.OK || result.body?.id !== target.id || result.body.error) throw new Constants.Error.Error("Page not replaced", `The server rejected the atomic replacement (${result?.status ?? "unknown"}). Existing page is retained; refresh staging before trying again.`);
			replacement = result.body; version = result.body.version;
		} catch (error) {
			// Read-only reconciliation, never a replay of the ambiguous mutation.
			const latest = await API.UploadSession.Get().catch(() => undefined), confirmed = latest?.body?.relationships.files?.find(row => row.id === target.id);
			if (latest?.status === StatusCodes.OK && latest.body?.id === uploadSession.id && confirmed?.file_hash === sha && confirmed.original_file_name === file.name) { replacement = confirmed; version = latest.body.version; toast.success("Replacement confirmed from current staging"); }
			else throw error;
		}
		if (!replacement || !Number.isInteger(version) || version < 1) throw new Constants.Error.Error("Replacement not confirmed", "Refresh owner staging to reconcile the current page. No request was replayed.");
		replacement.thumbnail = URL.createObjectURL(file); revokeThumbnail(target); pages[idx] = replacement; pages = pages; uploadSession.version = version;
	};
