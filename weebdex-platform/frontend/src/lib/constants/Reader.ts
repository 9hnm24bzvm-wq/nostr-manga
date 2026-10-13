export enum Scale {
	Original,
	LimitAll,
	LimitWidth,
	LimitHeight,
	StretchAll,
	StretchWidth,
	StretchHeight
}

export enum Layout {
	Paged,
	Scroll
}

export enum Direction {
	LTR,
	RTL
}

export enum Spread {
	Single,
	Double
}

export enum CommentsVisibility {
	Always,
	Collapsed,
	Hidden
}

export const CommentsText: Record<CommentsVisibility, string> = {
	[CommentsVisibility.Always]: "Always",
	[CommentsVisibility.Collapsed]: "Collapsed",
	[CommentsVisibility.Hidden]: "Hidden"
};
