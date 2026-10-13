	let lastSavedUpload = $state("");
	let resumeSaveTimer = 0;
	let resumeDisposed = false;
	const showResumeError = (error: unknown) => {
		if (error instanceof Constants.Error.Error) toast.error(error.title, { description: error.body });
		else toast.error("Upload could not be restored", { description: "Reload before creating another upload. Existing staging was kept." });
	};
	const restoreUpload = (saved: UploadSession) => {
		uploadSession = prepareResume(saved);
		draft = structuredClone(saved.draft ?? {});
		groups = saved.relationships.groups ?? [];
		isOneshot = !draft.volume && !draft.chapter;
		lastSavedUpload = signature(snapshot(draft, saved.relationships.files ?? []));
		if (saved.is_committed) {
			const result = saved.committed_result ?? saved.receipt;
			chapter = result?.relationships?.manga && !["accepted_pending_projection", "pending_projection"].includes(result.publication_status ?? "") ? result : undefined;
			message = { title: chapter ? "Saved chapter restored" : "Signed chapter accepted", description: chapter ? "This chapter has been materialized on this instance." : "The saved publication is waiting for local catalogue processing before it can be read." };
			isBeginAnotherUploadSession = false;
			isShowMessage = true;
		} else toast.info("Upload session restored", { description: resumeNotice(saved) });
	};
	const persistCurrentUpload = async () => {
		if (resumeDisposed || !uploadSession || uploadSession.can_mutate === false) return;
		const files = uploadSession.relationships.files ?? [];
		if (files.some(file => file.file || file.error || file.id.length !== 10)) return;
		const value = snapshot($state.snapshot(draft), files);
		if (signature(value) === lastSavedUpload) return;
		const saved = await saveResume(uploadSession, value);
		if (resumeDisposed || uploadSession?.id !== saved.id) return;
		acknowledge(uploadSession, saved);
		lastSavedUpload = signature(value);
	};
	const reconcileUploadFailure = async () => {
		try {
			const saved = await currentUpload();
			if (saved && (saved.manga_id ?? saved.relationships.manga?.id) === manga.id && (!saved.chapter_id || saved.is_committed) && (saved.can_mutate === false || !uploadSession)) restoreUpload(saved);
		} catch { /* Preserve original uncertainty; never replay a mutation. */ }
	};
	$effect(() => {
		if (!resumeReady || !uploadSession || uploadSession.can_mutate === false) return;
		const value = snapshot(draft, uploadSession.relationships.files ?? []);
		if (signature(value) === lastSavedUpload) return;
		window.clearTimeout(resumeSaveTimer);
		resumeSaveTimer = window.setTimeout(() => { void mutex.wait(persistCurrentUpload).catch(showResumeError); }, 500);
		return () => window.clearTimeout(resumeSaveTimer);
	});
	const hasUnsavedUpload = () => !!uploadSession && uploadSession.can_mutate !== false && signature(snapshot(draft, uploadSession.relationships.files ?? [])) !== lastSavedUpload;
	beforeNavigate(navigation => {
		if (!navigation.willUnload && hasUnsavedUpload()) {
			navigation.cancel();
			toast.info("Upload changes are still saving", { description: "Wait for the private staging save before leaving. No session was discarded." });
		}
	});
	onMount(() => {
		const warnUnsaved = (event: BeforeUnloadEvent) => { if (hasUnsavedUpload()) { event.preventDefault(); event.returnValue = ""; } };
		window.addEventListener("beforeunload", warnUnsaved);
		void mutex.wait(async () => {
			const saved = await currentUpload();
			if (resumeDisposed) return;
			if (saved) {
				if ((saved.manga_id ?? saved.relationships.manga?.id) !== manga.id || (saved.chapter_id && !saved.is_committed)) {
					toast.info("Existing upload kept", { description: "Opening its original editor instead of deleting staged work." });
					await goto(saved.chapter_id ? `/chapter/${saved.chapter_id}/edit` : `/title/${saved.manga_id ?? saved.relationships.manga!.id}/upload`);
					return;
				}
				restoreUpload(saved);
			}
			resumeReady = true;
		}).catch(showResumeError);
		return () => { resumeDisposed = true; window.clearTimeout(resumeSaveTimer); window.removeEventListener("beforeunload", warnUnsaved); };
	});
