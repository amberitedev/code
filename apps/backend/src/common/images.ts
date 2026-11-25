/** How an uploaded image is served: each upload has its own URL, and its type is never sniffed. */
export const IMAGE_HEADERS = {
	'Cache-Control': 'public, max-age=31536000, immutable',
	'X-Content-Type-Options': 'nosniff',
}

/** The type of a PNG, JPEG, GIF or WebP image by its first bytes, or undefined for anything else. */
export function imageType(bytes: Uint8Array): string | undefined {
	const text = (start: number, end: number) => String.fromCharCode(...bytes.subarray(start, end))
	if (bytes[0] === 137 && text(1, 4) === 'PNG') return 'image/png'
	if (bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255) return 'image/jpeg'
	if (/^GIF8[79]a$/.test(text(0, 6))) return 'image/gif'
	if (text(0, 4) === 'RIFF' && text(8, 12) === 'WEBP') return 'image/webp'
	return undefined
}
