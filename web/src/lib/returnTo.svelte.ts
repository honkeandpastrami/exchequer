// Where an editor goes back to: the list it was opened from, filters included.

let last = $state('/transactions');

export const returnTo = {
	get transactions(): string {
		return last;
	},
	/** The list page records itself on every change of filters. */
	set transactions(path: string) {
		last = path;
	}
};
