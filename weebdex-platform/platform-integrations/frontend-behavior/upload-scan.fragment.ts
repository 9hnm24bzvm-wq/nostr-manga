	const scanFiles = async (entry?: FileSystemEntry | null, budget = { files: 0, entries: 0, bytes: 0 }, depth = 0): Promise<File[]> => {
		if (!entry) return [];
		if (++budget.entries > 2048 || depth > 16) throw new Constants.Error.Error("Folder exceeds upload limits", "Limit the selection to 500 files, 2048 entries and 16 nested directories.");
		if (entry.isFile) {
			if (++budget.files > maxSessionFiles) throw new Constants.Error.Error("Too many upload files", "A selection admits at most 500 files.");
			const file = await new Promise<File>((resolve, reject) => (<FileSystemFileEntry>entry).file(resolve, reject));
			budget.bytes += file.size;
			if (budget.bytes > maxSessionSize) throw new Constants.Error.Error("Folder exceeds upload limits", "Selected input exceeds 500 MiB.");
			return [file];
		}
		if (!entry.isDirectory) return [];
		const reader = (entry as FileSystemDirectoryEntry).createReader(), files: File[] = [];
		while (true) {
			const entries = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
			if (!entries.length) break;
			for (const child of entries) files.push(...await scanFiles(child, budget, depth + 1));
		}
		return files;
	};
