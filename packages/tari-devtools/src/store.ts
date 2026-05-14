// store.ts: in-memory ring buffers for CDP event streams and interceptor data.

export interface ConsoleEntry {
	time: number;
	level: string;
	message: string;
	count?: number;
}

export interface ExceptionEntry {
	time: number;
	message: string;
	stack: string[];
}

export interface NetworkEntry {
	id: string;
	method: string;
	url: string;
	startTime: number;
	endTime?: number;
	status: number;
	failed: boolean;
	errorText?: string;
}

class RingBuffer<T> {
	private items: T[] = [];

	constructor(readonly capacity: number) {}

	push(item: T): void {
		this.items.push(item);
		if (this.items.length > this.capacity) this.items.shift();
	}

	all(): readonly T[] {
		return this.items;
	}

	get size(): number {
		return this.items.length;
	}

	clear(): void {
		this.items = [];
	}
}

class DevToolsStore {
	private exceptions = new RingBuffer<ExceptionEntry>(100);
	private network = new Map<string, NetworkEntry>();
	private networkOrder: string[] = [];
	static readonly MAX_NETWORK = 200;

	addException(entry: ExceptionEntry): void {
		this.exceptions.push(entry);
	}

	getExceptions(): ExceptionEntry[] {
		return [...this.exceptions.all()];
	}

	addNetwork(entry: NetworkEntry): void {
		if (this.networkOrder.length >= DevToolsStore.MAX_NETWORK) {
			const oldest = this.networkOrder.shift()!;
			this.network.delete(oldest);
		}
		this.network.set(entry.id, entry);
		this.networkOrder.push(entry.id);
	}

	updateNetwork(id: string, updates: Partial<NetworkEntry>): void {
		const e = this.network.get(id);
		if (e) Object.assign(e, updates);
	}

	getNetwork(): NetworkEntry[] {
		return this.networkOrder
			.map((id) => this.network.get(id))
			.filter((e): e is NetworkEntry => e !== undefined);
	}

	counts(): { console: number; exceptions: number; network: number } {
		return { console: 0, exceptions: this.exceptions.size, network: this.networkOrder.length };
	}

	clear(): void {
		this.exceptions.clear();
		this.network.clear();
		this.networkOrder.length = 0;
	}
}

export const store = new DevToolsStore();
