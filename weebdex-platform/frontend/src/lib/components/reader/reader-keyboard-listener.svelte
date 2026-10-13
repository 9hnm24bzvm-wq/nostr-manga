<script lang="ts">
	import { Constants } from "$lib";
	import { getIsOpen } from "$lib/components/ReportDialog.svelte";
	import { useReader } from "./context.svelte";

	const reader = useReader();
	const readerPrefs = reader.prefs;

	let raf = 0;
	const scrollBy = (n: number) => {
		raf = requestAnimationFrame(() => {
			window.scrollBy({ top: n, behavior: "smooth" });
			if (raf > 0) scrollBy(n);
		});
	};

	const onKeyDown = (ev: KeyboardEvent) => {
		if (
			ev.target instanceof HTMLInputElement ||
			ev.target instanceof HTMLTextAreaElement ||
			ev.ctrlKey ||
			reader.scroll.locked ||
			getIsOpen()
		) {
			return;
		}

		const { code } = ev;
		if (
			[
				"ArrowUp",
				"KeyW",
				"KeyI",
				"ArrowDown",
				"KeyS",
				"KeyK",
				"ArrowLeft",
				"KeyA",
				"KeyJ",
				"ArrowRight",
				"KeyD",
				"KeyL",
				"Period",
				"Slash",
				"KeyQ",
				"BracketLeft",
				"KeyE",
				"BracketRight",
				"Semicolon",
				"Quote"
			].includes(code)
		) {
			ev.preventDefault();
			ev.stopPropagation();
			ev.stopImmediatePropagation();
		}

		switch (code) {
			case "ArrowUp":
			case "KeyW":
			case "KeyI":
				if (raf == 0) {
					scrollBy(-$readerPrefs.scroll);
				}
				break;
			case "ArrowDown":
			case "KeyS":
			case "KeyK":
				if (raf === 0) {
					scrollBy($readerPrefs.scroll);
				}
				break;
			case "ArrowLeft":
			case "KeyA":
			case "KeyJ":
				if ($readerPrefs.layout !== Constants.Reader.Layout.Scroll || $readerPrefs.keyboard_navigation) {
					if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.gotoNextPage();
					else reader.gotoPreviousPage();
				}
				break;
			case "ArrowRight":
			case "KeyD":
			case "KeyL":
				if ($readerPrefs.layout !== Constants.Reader.Layout.Scroll || $readerPrefs.keyboard_navigation) {
					if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.gotoPreviousPage();
					else reader.gotoNextPage();
				}
				break;
			case "Period":
				if ($readerPrefs.layout !== Constants.Reader.Layout.Scroll || $readerPrefs.keyboard_navigation) {
					if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.changePage(reader.pages.length - 1);
					else reader.changePage(0);
				}
				break;
			case "Slash":
				if ($readerPrefs.layout !== Constants.Reader.Layout.Scroll || $readerPrefs.keyboard_navigation) {
					if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.changePage(0);
					else reader.changePage(reader.pages.length - 1);
				}
				break;
			case "KeyQ":
			case "BracketLeft":
				if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.gotoNextChapter();
				else reader.gotoPreviousChapter();
				break;
			case "KeyE":
			case "BracketRight":
				if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.gotoPreviousChapter();
				else reader.gotoNextChapter();
				break;
			case "Semicolon":
				if ($readerPrefs.direction === Constants.Reader.Direction.RTL) reader.changeChapter(0);
				else if (reader.aggregated?.chapters.length) reader.changeChapter(reader.aggregated.chapters.length - 1);
				break;
			case "Quote":
				if ($readerPrefs.direction === Constants.Reader.Direction.RTL) {
					if (reader.aggregated?.chapters.length) reader.changeChapter(reader.aggregated.chapters.length - 1);
				} else reader.changeChapter(0);
				break;
		}
	};

	const onKeyUp = () => {
		window.cancelAnimationFrame(raf);
		raf = 0;
	};
</script>

<svelte:window onkeydown={onKeyDown} onkeyup={onKeyUp} />
