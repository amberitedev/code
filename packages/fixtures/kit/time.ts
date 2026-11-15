/** A time that many days from now, as the backend stores times. Negative is in the past. */
export function days(count: number): string {
	return new Date(Date.now() + count * 86_400_000).toISOString()
}
