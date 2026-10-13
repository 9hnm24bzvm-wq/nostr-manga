import { getContext, setContext } from "svelte";

export type CommentsProps = {
	thread: Thread;

	stats: () => ThreadStats;
	setStats: (stats: ThreadStats) => void;
	voteState: () => number | undefined;
	setVoteState: (voteState: number | undefined) => void;

	feed: () => ReplyFeedResponse | undefined;
	setFeed: (feed: ReplyFeedResponse | undefined) => void;
};

class CommentsState {
	readonly props: CommentsProps;
	ref = $state<HTMLElement>(null!);

	textAreaRef = $state<HTMLTextAreaElement>(null!);
	body = $state("");

	editTarget = $state<Reply>();
	isEdit = $state(false);

	constructor(props: CommentsProps) {
		this.props = props;
	}

	get thread() {
		return this.props.thread;
	}

	get stats() {
		return this.props.stats();
	}

	get voteState() {
		return this.props.voteState();
	}

	set stats(stats: ThreadStats) {
		this.props.setStats(stats);
	}

	set voteState(voteState: number | undefined) {
		this.props.setVoteState(voteState);
	}

	get feed() {
		return this.props.feed();
	}

	set feed(feed: ReplyFeedResponse | undefined) {
		this.props.setFeed(feed);
	}

	async quote(reply: Reply) {
		const result: string[] = [];
		const lines = reply.body?.split("\n");
		if (lines?.length) {
			let isPreviousBlank = false;
			for (let i = 0; i < lines.length; i++) {
				let line = lines[i];
				if (line.startsWith("> ")) {
					continue;
				}
				line = line.trim();

				if (line.length) {
					isPreviousBlank = false;
				} else {
					if (isPreviousBlank || !result.length) {
						continue;
					}
					isPreviousBlank = true;
				}
				result.push("> " + line);
			}
		}

		if (this.body.trim()) {
			if (this.body.endsWith("\n")) {
				this.body += "\n";
			} else this.body += "\n\n";
		}
		this.body += `> [quote=${reply.relationships.author?.name},user=${reply.relationships.author?.id},post=${reply.id}]\n`;
		this.body += result.join("\n") + "\n";
		this.textAreaRef?.focus();
	}

	async edit(reply: Reply) {
		this.editTarget = reply;
		this.isEdit = true;
	}
}

const SYMBOL_KEY = "comments";

export const setComments = (props: CommentsProps) => {
	return setContext<CommentsState>(SYMBOL_KEY, new CommentsState(props));
};

export const useComments = () => {
	return getContext<CommentsState>(SYMBOL_KEY);
};
