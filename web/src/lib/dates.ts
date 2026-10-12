/** Today in the browser's time zone, as `YYYY-MM-DD`. */
export function today(): string {
	return new Date().toLocaleDateString('en-CA');
}
