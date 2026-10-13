	let resumeReady = $state(false);
	let lastSavedUpload = $state("");
	let resumeSaveTimer = 0;
	let resumeDisposed = false;
	const showResumeError = (error: unknown) => {
		if (error instanceof Constants.Error.Error) toast.error(error.title, { description: error.body });
		else toast.error("Upload could not be restored", { description: "Existing staging was kept. Reload its current state before retrying." });
	};
	const restoreUpload = (saved: UploadSession) => {
		uploadSession = prepareResume(saved);
		lastSavedUpload = signature(snapshot(saved.draft ?? chapter, saved.relationships.files ?? []));
		toast.info("Upload session restored", { description: resumeNotice(saved) });
	};
	const persistCurrentUpload = async () => {
		if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;
		const files = uploadSession.relationships.files ?? [];
		if (files.some(file => file.file || file.error || file.id.length !== 10)) return;
		const value = snapshot(chapter, files);
		if (signature(value) === lastSavedUpload) return;
		const saved = await saveResume(uploadSession, value);
		if (resumeDisposed || uploadSession?.id !== saved.id) return;
		acknowledge(uploadSession, saved); lastSavedUpload = signature(value);
	};
	$effect(() => {
		if (!resumeReady || !uploadSession || uploadSession.can_mutate === false) return;
		if (signature(snapshot(chapter, uploadSession.relationships.files ?? [])) === lastSavedUpload) return;
		window.clearTimeout(resumeSaveTimer);
		resumeSaveTimer = window.setTimeout(() => { void mutex.wait(persistCurrentUpload).catch(showResumeError); }, 500);
		return () => window.clearTimeout(resumeSaveTimer);
	});
	const hasUnsavedUpload = () => !!uploadSession && uploadSession.can_mutate !== false && signature(snapshot(chapter, uploadSession.relationships.files ?? [])) !== lastSavedUpload;
	beforeNavigate(navigation => {
		if (!navigation.willUnload && hasUnsavedUpload()) { navigation.cancel(); toast.info("Upload changes are still saving", { description: "Wait for the private staging save before leaving." }); }
	});
	onMount(() => {
		const warnUnsaved = (event: BeforeUnloadEvent) => { if (hasUnsavedUpload()) { event.preventDefault(); event.returnValue = ""; } };
		window.addEventListener("beforeunload", warnUnsaved);
		void mutex.wait(async () => {
			await onReset();
			const saved = await currentUpload();
			if (resumeDisposed) return;
			if (saved?.chapter_id === chapter.id) restoreUpload(saved);
			resumeReady = true;
		}).catch(showResumeError);
		return () => { resumeDisposed = true; window.clearTimeout(resumeSaveTimer); window.removeEventListener("beforeunload", warnUnsaved); };
	});
