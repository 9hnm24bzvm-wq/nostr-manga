	const inspectImageDimensions = async (file: File) => {
		const bytes = new Uint8Array(await file.slice(0, 1024 * 1024).arrayBuffer());
		const view = new DataView(bytes.buffer);
		let width = 0, height = 0;
		if (bytes.length >= 24 && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71) { width = view.getUint32(16); height = view.getUint32(20); }
		else if (bytes.length >= 10 && String.fromCharCode(...bytes.subarray(0, 3)) === "GIF") { width = view.getUint16(6, true); height = view.getUint16(8, true); }
		else if (bytes.length >= 4 && bytes[0] === 255 && bytes[1] === 216) {
			let offset = 2;
			while (offset + 4 <= bytes.length) {
				if (bytes[offset++] !== 255) break;
				while (bytes[offset] === 255) offset++;
				const marker = bytes[offset++];
				if (marker === 217 || marker === 218) break;
				if (marker === 1 || marker >= 208 && marker <= 215) continue;
				if (offset + 2 > bytes.length) break;
				const length = view.getUint16(offset);
				if (length < 2 || offset + length > bytes.length) break;
				if (marker >= 192 && marker <= 207 && ![196, 200, 204].includes(marker) && length >= 7) { height = view.getUint16(offset + 3); width = view.getUint16(offset + 5); break; }
				offset += length;
			}
		}
		if (!width || !height || width * height > 50_000_000) throw new Constants.Error.Error(file.name, "Could not decode image, or its decoded dimensions exceed the 50-million-pixel admission limit.");
	};
	const decodeUploadImage = async (file: File) => {
		await inspectImageDimensions(file);
		return new Promise<HTMLImageElement>((resolve, reject) => {
			const image = new Image(), url = URL.createObjectURL(file);
			const cleanup = () => { image.onload = image.onerror = null; window.clearTimeout(timer); URL.revokeObjectURL(url); };
			const timer = window.setTimeout(() => { cleanup(); image.src = ""; reject(new Constants.Error.Error(file.name, "Could not decode image before the 30-second deadline.")); }, 30000);
			image.onload = () => { cleanup(); resolve(image); };
			image.onerror = () => { cleanup(); reject(new Constants.Error.Error(file.name, "Could not decode image.")); };
			image.src = url;
		});
	};
	const processFiles = async (files: File[]) => {
		if (!uploadSession || uploadSession.can_mutate === false || disabled) return;
		if (files.length > maxSessionFiles || pages.length + files.length > maxSessionFiles) throw new Constants.Error.Error("Too many upload files", "A session admits at most 500 pages. Split large folders into smaller selections.");
		const admittedSessionSize = pages.reduce((total, page) => total + Number(page.file_size ?? page.file?.size ?? 0), 0);
		let remaining = maxSessionSize - admittedSessionSize;
		const processedFiles: File[] = [];
		// One decoder at a time bounds memory; no Promise.all over untrusted files.
		const admit = async (file: File) => {
			if (file.size > maxFileSize) throw new Constants.Error.Error(file.name, "File too large");
			const decoded = await decodeUploadImage(file);
			const pieces = decoded.width > maxImageWidth || decoded.height > maxImageHeight ? await splitImage(decoded, file.name) : [file];
			for (const piece of pieces) {
				if (pages.length + processedFiles.length >= maxSessionFiles || piece.size > maxFileSize || piece.size > remaining) throw new Constants.Error.Error("Upload limit exceeded", "The decoded selection exceeds 500 pages or the 500 MiB session budget.");
				remaining -= piece.size; processedFiles.push(piece);
			}
		};
		for (const file of files) {
			if (imageMimeTypes.includes(file.type)) await admit(file);
			else if (archiveMimeTypes.includes(file.type) || /\.(cbz|zip)$/i.test(file.name)) {
				const extracted = await extractArchive(file, remaining, maxSessionFiles - pages.length - processedFiles.length);
				for (const image of extracted) await admit(image);
			}
		}
