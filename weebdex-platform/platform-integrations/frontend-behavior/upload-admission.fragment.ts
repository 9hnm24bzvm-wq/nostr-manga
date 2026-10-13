	const maxSessionFiles = 500;
	const maxArchiveCompressedSize = 100 * 1024 * 1024;
	const inspectArchiveDirectory = async (file: File) => {
		// Admit standard single-disk ZIP centrally, before JSZip allocates entry objects.
		const start = Math.max(0, file.size - 65557);
		const tail = new Uint8Array(await file.slice(start).arrayBuffer());
		const view = new DataView(tail.buffer);
		let end = -1;
		for (let offset = tail.length - 22; offset >= 0; offset--) {
			if (view.getUint32(offset, true) === 0x06054b50 && offset + 22 + view.getUint16(offset + 20, true) === tail.length) { end = offset; break; }
		}
		if (end < 0) throw new Constants.Error.Error(file.name, "Failed to extract archive: no bounded standard ZIP directory.");
		// JSZip chooses the final EOCD signature; reject any competing marker after ours.
		for (let cursor = end + 4; cursor + 4 <= tail.length; cursor++) {
			if (view.getUint32(cursor, true) === 0x06054b50) throw new Constants.Error.Error(file.name, "Ambiguous ZIP end-directory markers are not supported by the bounded importer.");
		}
		const count = view.getUint16(end + 10, true), size = view.getUint32(end + 12, true), offset = view.getUint32(end + 16, true);
		if (view.getUint16(end + 4, true) || view.getUint16(end + 6, true) || view.getUint16(end + 8, true) !== count || count === 65535 || size === 0xffffffff || offset === 0xffffffff)
			throw new Constants.Error.Error(file.name, "Split-disk and ZIP64 archives are not supported by the bounded browser importer. Use a standard single-disk CBZ.");
		if (count > 2048 || size > 8 * 1024 * 1024 || offset + size !== start + end)
			throw new Constants.Error.Error(file.name, "Archive directory exceeds the bounded entry/metadata admission limit.");
		const directory = new Uint8Array(await file.slice(offset, offset + size).arrayBuffer()), central = new DataView(directory.buffer);
		let cursor = 0;
		for (let index = 0; index < count; index++) {
			if (cursor + 46 > directory.length || central.getUint32(cursor, true) !== 0x02014b50) throw new Constants.Error.Error(file.name, "Failed to extract archive: invalid central directory.");
			const compressed = central.getUint32(cursor + 20, true), expanded = central.getUint32(cursor + 24, true), local = central.getUint32(cursor + 42, true);
			if (central.getUint16(cursor + 34, true) || [compressed, expanded, local].includes(0xffffffff)) throw new Constants.Error.Error(file.name, "ZIP64 and split-disk entries are not supported.");
			if (local >= offset || compressed > file.size || expanded > maxSessionSize) throw new Constants.Error.Error(file.name, "Archive entry exceeds the bounded size admission limit.");
			cursor += 46 + central.getUint16(cursor + 28, true) + central.getUint16(cursor + 30, true) + central.getUint16(cursor + 32, true);
			if (cursor > directory.length) throw new Constants.Error.Error(file.name, "Failed to extract archive: truncated central directory.");
		}
		if (cursor !== directory.length) throw new Constants.Error.Error(file.name, "Unsupported trailing ZIP directory metadata.");
	};
	const extractArchive = async (file: File, byteBudget = maxSessionSize, countBudget = maxSessionFiles) => {
		if (file.size > maxArchiveCompressedSize) throw new Constants.Error.Error(file.name, "Archive exceeds the 100 MiB compressed admission limit. Split it into smaller archives.");
		await inspectArchiveDirectory(file);
		try {
			const zip = await JSZip.loadAsync(file);
			const all = Object.values(zip.files);
			if (all.length > 2048) throw new Constants.Error.Error(file.name, "Archive has too many entries.");
			const entries = all.filter(entry => !entry.dir && /\.(jpg|jpeg|png|gif)$/i.test(entry.name)).sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }));
			if (entries.length > countBudget) throw new Constants.Error.Error(file.name, "Archive exceeds the remaining 500-page session limit.");
			const files: File[] = [];
			let remaining = byteBudget;
			for (const entry of entries) {
				// Stream decompressed output and stop before accumulating a ZIP bomb.
				const blob = await new Promise<Blob>((resolve, reject) => {
					const stream = (entry as any).internalStream("uint8array");
					let chunks: Uint8Array[] = [], size = 0, finished = false;
					const fail = (error: unknown) => { if (finished) return; finished = true; stream.pause(); chunks = []; reject(error); };
					stream.on("data", (chunk: Uint8Array) => {
						if (finished) return;
						if (size + chunk.byteLength > maxFileSize || chunk.byteLength > remaining) return fail(new Constants.Error.Error(entry.name, "Decompressed image or aggregate archive output exceeds the upload limit."));
						size += chunk.byteLength; remaining -= chunk.byteLength; chunks.push(new Uint8Array(chunk));
					}).on("error", fail).on("end", () => { if (!finished) { finished = true; resolve(new Blob(chunks as BlobPart[])); chunks = []; } }).resume();
				});
				const ext = entry.name.toLowerCase().split(".").pop();
				const type = ext === "png" ? "image/png" : ext === "gif" ? "image/gif" : "image/jpeg";
				files.push(new File([blob], entry.name, { type }));
			}
			return files;
		} catch (error) {
			if (error instanceof Constants.Error.Error) throw error;
			throw new Constants.Error.Error(file.name, "Failed to extract archive");
		}
	};
