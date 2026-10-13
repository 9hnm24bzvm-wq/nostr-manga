import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { Constants, Mutex, Utils } from "$lib";
import type { PersistentStore } from "$lib/hooks";
import { skipEndOfChapterPage } from "$lib/stores";
import { getContext, setContext, tick } from "svelte";
import { get } from "svelte/store";

export type ReaderStateProps = {
	prefs: PersistentStore<ReaderPreferences>;
	chapter: Chapter;
	aggregated: () => ChapterAggregateResponse | undefined;
};

let isNavigateLastPage = false;

class ReaderState {
	readonly props: ReaderStateProps;
	readonly mutex = new Mutex();
	readonly aggregated = $derived.by(() => this.props.aggregated());
	key = $state(0);

	scroll = $state<ReaderScrollState>({ locked: false, scrolling: false });
	headroom = $state<Headroom>();

	readonly chapterIndex = $derived.by(() => this.aggregated?.chapters.findIndex(e => this.chapter.id in e.entries));
	readonly firstChapter = $derived.by(() =>
		this.chapterIndex !== undefined && this.aggregated?.chapters
			? this.findChapter(this.aggregated.chapters.length - 1)
			: undefined
	);
	readonly lastChapter = $derived.by(() =>
		this.chapterIndex !== undefined && this.aggregated?.chapters ? this.findChapter(0) : undefined
	);
	readonly prevChapter = $derived.by(() =>
		this.chapterIndex !== undefined && this.aggregated?.chapters ? this.findChapter(this.chapterIndex + 1) : undefined
	);
	readonly nextChapter = $derived.by(() =>
		this.chapterIndex !== undefined && this.aggregated?.chapters ? this.findChapter(this.chapterIndex - 1) : undefined
	);

	pages = $state<ReaderPage[]>([]);
	mainRef = $state<HTMLElement>(null!);
	currentPage = $state<ReaderPage | undefined>();

	spreads = $state<ReaderPage[][]>([]);
	currentSpread = $derived.by(() => {
		if (this.currentPage && this.spreads) {
			return this.spreads.find(e => e.some(f => f.index === this.currentPage?.index)) ?? [];
		}
		return this.currentPage ? [this.currentPage] : [];
	});

	isViewingLastPage = $state(false);
	lastPageRef = $state<HTMLElement>(null!);
	isShowComments = $state(false);

	constructor(props: ReaderStateProps) {
		this.props = props;
		this.isViewingLastPage = isNavigateLastPage;
		isNavigateLastPage = false;
	}

	get prefs() {
		return this.props.prefs;
	}

	get chapter() {
		return this.props.chapter;
	}

	findChapter(idx: number): ReaderChapterInfo | undefined {
		if (!this.aggregated?.chapters || idx < 0 || idx >= this.aggregated.chapters.length) {
			return;
		}

		const aggregate = this.aggregated.chapters[idx];
		const entries = Object.entries(aggregate.entries);
		if (entries.length > 1) {
			const currentGroups = this.chapter.relationships.groups?.map(e => e.name) ?? ["No Group"];
			const scores: { id: string; entry: ChapterAggregateEntry; score: number }[] = [];

			for (const [id, entry] of entries) {
				const groups = entry.groups.map(idx => this.aggregated?.groups[idx].name);
				const score = groups.filter(e => currentGroups.includes(e ?? "")).length;
				scores.push({ id, entry, score });
			}
			scores.sort((a, b) => b.score - a.score);

			if (scores[0].id !== this.chapter.id) {
				return {
					id: scores[0].id,
					volume: aggregate.volume,
					chapter: aggregate.chapter,
					language: this.aggregated?.languages[scores[0].entry.language],
					groups: scores[0].entry.groups.map(idx => this.aggregated?.groups[idx].name ?? "")
				};
			}
		} else if (entries[0][0] !== this.chapter.id) {
			return {
				id: entries[0][0],
				volume: aggregate.volume,
				chapter: aggregate.chapter,
				language: this.aggregated?.languages[entries[0][1].language],
				groups: entries[0][1].groups.map(idx => this.aggregated?.groups[idx].name ?? "")
			};
		}
	}

	async changeChapter(idx: number) {
		const result = this.findChapter(idx);
		if (result) {
			await goto(resolve(`/chapter/${result.id}/1`));
		}
	}
	async gotoPreviousChapter() {
		return this.chapterIndex !== undefined && this.changeChapter(this.chapterIndex + 1);
	}
	async gotoNextChapter() {
		return this.chapterIndex !== undefined && this.changeChapter(this.chapterIndex - 1);
	}

	async changePage(idx: number) {
		return this.mutex.exec(async () => {
			if (
				!this.pages.length ||
				!this.currentPage ||
				idx === this.currentPage.index ||
				idx < 0 ||
				idx > this.pages.length - 1
			) {
				return;
			}
			this.isViewingLastPage = false;

			this.currentPage = this.pages[idx];
			await this.scrollIntoView(this.currentPage);
		});
	}
	async gotoPreviousPage() {
		return this.mutex.exec(async () => {
			if (!this.pages.length || !this.currentPage) {
				return;
			}

			if (this.isViewingLastPage) {
				this.isViewingLastPage = false;
				if (!get(skipEndOfChapterPage) && get(this.prefs).layout === Constants.Reader.Layout.Paged) {
					return;
				}
			}

			if (get(this.prefs).double ? this.currentSpread === this.spreads[0] : this.currentPage === this.pages[0]) {
				if (this.chapterIndex !== undefined) {
					if (this.chapterIndex === Number(this.aggregated?.chapters.length) - 1) {
						await goto(
							resolve(
								`/title/${this.chapter.relationships.manga.id}${Utils.makeSlug(this.chapter.relationships.manga.title)}`
							)
						);
					} else if (get(this.prefs).auto_advance) {
						isNavigateLastPage = true;
						await this.gotoPreviousChapter();
					}
				}
			} else {
				let prevIdx = this.currentPage.index - 1;
				if (get(this.prefs).double && this.spreads && this.currentSpread) {
					const spreadIdx = this.spreads.indexOf(this.currentSpread);
					if (spreadIdx > 0) {
						prevIdx = this.spreads[spreadIdx - 1][0].index;
					}
				}

				this.currentPage = this.pages[Math.max(prevIdx, 0)];
				await this.scrollIntoView(this.currentPage);
			}
		});
	}
	async gotoNextPage() {
		return this.mutex.exec(async () => {
			if (!this.pages.length || !this.currentPage) {
				return;
			}

			if (
				get(this.prefs).double
					? this.currentSpread === this.spreads[this.spreads.length - 1]
					: this.currentPage === this.pages[this.pages.length - 1]
			) {
				if (this.chapterIndex !== undefined) {
					if (this.isViewingLastPage || get(skipEndOfChapterPage)) {
						if (this.chapterIndex === 0) {
							await goto(
								resolve(
									`/title/${this.chapter.relationships.manga.id}${Utils.makeSlug(this.chapter.relationships.manga.title)}`
								)
							);
						} else if (get(this.prefs).auto_advance) {
							await this.gotoNextChapter();
						}
					} else {
						this.isViewingLastPage = true;
						await this.scrollIntoView(this.currentPage);
					}
				}
			} else {
				let nextIdx = this.currentPage.index + 1;
				if (get(this.prefs).double && this.spreads && this.currentSpread) {
					const spreadIdx = this.spreads.indexOf(this.currentSpread);
					if (spreadIdx < this.spreads.length - 1) {
						nextIdx = this.spreads[spreadIdx + 1][0].index;
					}
				}

				this.currentPage = this.pages[Math.min(nextIdx, this.pages.length - 1)];
				await this.scrollIntoView(this.currentPage);
			}
		});
	}

	readonly listener = $derived((ev: MouseEvent) => {
		if (!this.mainRef) {
			return;
		}

		let clientX = ev.clientX;
		if (get(this.prefs).panel_position === "left") {
			clientX -= 450;
		}

		if (get(this.prefs).tap_navigation || get(this.prefs).layout !== Constants.Reader.Layout.Scroll) {
			let isPrevious = clientX <= this.mainRef.clientWidth / 2;
			if (get(this.prefs).direction === Constants.Reader.Direction.RTL) {
				isPrevious = !isPrevious;
			}

			if (isPrevious) this.gotoPreviousPage();
			else this.gotoNextPage();
		}
	});

	async scrollIntoView(page: ReaderPage) {
		window.requestAnimationFrame(async () => {
			await tick();
			if (page.ref) {
				const pageRef = this.isViewingLastPage ? this.lastPageRef : get(page.ref);
				if (get(this.prefs).layout === Constants.Reader.Layout.Paged) {
					this.mainRef.scrollIntoView();
				} else if (pageRef) {
					if (pageRef.clientHeight < window.innerHeight) {
						pageRef.scrollIntoView({ block: "center" });
					} else pageRef.scrollIntoView();
				}
			}
		});
	}
}

const SYMBOL_KEY = "reader";

export const setReader = (props: ReaderStateProps) => {
	return setContext<ReaderState>(Symbol.for(SYMBOL_KEY), new ReaderState(props));
};

export const useReader = () => {
	return getContext<ReaderState>(Symbol.for(SYMBOL_KEY));
};
