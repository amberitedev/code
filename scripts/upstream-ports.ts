// The record of Modrinth changes we had to carry by hand into code we rewrote. It lives in commit
// messages, so it cannot drift from the history it describes. A sync's merge commit lists what it
// owes under "To port:", and a trailer on any later commit settles one:
//
//   Upstream: <sha>               this commit carries the change into our code
//   Upstream-skip: <sha> <why>    nothing of ours needs it
//   Upstream-later: <sha> <why>   still owed, and every later sync says so

export type Port = {
	sha: string
	title: string
	// `open` has no trailer yet. `later` stays owed until a port or a skip names it.
	state: 'open' | 'later' | 'skipped' | 'ported'
	// The reason given with a skip or a later.
	note: string
}

// A weaker trailer never undoes a stronger one.
const STATES = ['open', 'later', 'skipped', 'ported'] as const

// The body of a merge commit that owes `commits`, each `<sha> <title>`.
export function owing(commits: string[]) {
	return commits.length > 0 ? `\nTo port:\n${commits.join('\n')}\n` : ''
}

// Every owed change with what became of it. `messages` are whole commit messages, in any order.
export function readPorts(messages: string[]): Port[] {
	const ports: Port[] = []
	const trailers: Pick<Port, 'sha' | 'state' | 'note'>[] = []
	for (const message of messages) {
		const owed = /^To port:\n((?:.+\n?)*)/m.exec(message)?.[1] ?? ''
		for (const line of owed.split('\n').filter(Boolean)) {
			const [sha = '', ...title] = line.split(' ')
			ports.push({ sha, title: title.join(' '), state: 'open', note: '' })
		}
		for (const match of message.matchAll(/^Upstream(|-skip|-later): ([0-9a-f]{7,40})\b ?(.*)$/gm)) {
			const [, kind, sha = '', note = ''] = match
			const state = kind === '-skip' ? 'skipped' : kind === '-later' ? 'later' : 'ported'
			trailers.push({ sha, state, note })
		}
	}
	for (const port of ports) {
		for (const trailer of trailers) {
			// Either side may be abbreviated.
			const same = trailer.sha.startsWith(port.sha) || port.sha.startsWith(trailer.sha)
			if (same && STATES.indexOf(trailer.state) > STATES.indexOf(port.state)) {
				Object.assign(port, { state: trailer.state, note: trailer.note })
			}
		}
	}
	return ports
}
