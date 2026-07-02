import { WebSocket } from "ws";
import { store } from "./store.js";

const CDP_HOST = "http://localhost:9222";

const INTERCEPTOR = `(function() {
	if (window.__mcp_intercepted) return;
	window.__mcp_intercepted = true;
	window.__mcp_logs = window.__mcp_logs || [];
	window.__mcp_errs = window.__mcp_errs || [];
	const _c = console;
	['log','info','warn','error','debug'].forEach(function(lvl) {
		const orig = _c[lvl].bind(_c);
		_c[lvl] = function() {
			const msg = Array.from(arguments).map(function(a) {
				try { return typeof a === 'object' ? JSON.stringify(a) : String(a); } catch(e) { return String(a); }
			}).join(' ');
			window.__mcp_logs.push({ time: Date.now(), level: lvl, message: msg });
			if (window.__mcp_logs.length > 500) window.__mcp_logs.shift();
			orig.apply(_c, arguments);
		};
	});
})();`;

export class CdpClient {
	private ws: WebSocket | null = null;
	private cmdId = 1;
	private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
	private eventHandlers = new Map<string, ((params: unknown) => void)[]>();
	connected = false;
	targetUrl = "";
	targetTitle = "";
	interceptorInjected = false;

	async connect(): Promise<void> {
		const res = await fetch(`${CDP_HOST}/json`);
		if (!res.ok) throw new Error(`CDP list failed: ${res.status}`);
		const targets = (await res.json()) as Array<{ webSocketDebuggerUrl: string; url: string; title: string; type: string }>;
		const target = targets.find((t) => t.type === "page") ?? targets[0];
		if (!target) throw new Error("No CDP targets found");

		this.targetUrl = target.url ?? "";
		this.targetTitle = target.title ?? "";

		await new Promise<void>((resolve, reject) => {
			const ws = new WebSocket(target.webSocketDebuggerUrl);
			ws.on("open", () => {
				this.ws = ws;
				this.connected = true;
				resolve();
			});
			ws.on("error", reject);
			ws.on("message", (data: Buffer) => this.onMessage(data));
			ws.on("close", () => {
				this.connected = false;
				this.interceptorInjected = false;
			});
		});

		await this.request("Runtime.enable", {});
		await this.request("Network.enable", {});
		await this.request("Page.enable", {});
		await this.request("Performance.enable", {});

		this.on("Runtime.exceptionThrown", (p) => this.onException(p as { exceptionDetails: { text: string; stackTrace?: { callFrames: Array<{ url: string; lineNumber: number; functionName: string }> } } }));
		this.on("Network.requestWillBeSent", (p) => this.onRequest(p as { requestId: string; request: { url: string; method: string }; timestamp: number }));
		this.on("Network.responseReceived", (p) => this.onResponse(p as { requestId: string; response: { status: number; mimeType: string }; timestamp: number }));
		this.on("Network.loadingFailed", (p) => this.onFailed(p as { requestId: string; errorText: string; timestamp: number }));
		this.on("Page.frameNavigated", () => this.injectInterceptor());

		await this.injectInterceptor();
	}

	async ensureConnected(): Promise<boolean> {
		if (this.connected) return true;
		try { await this.connect(); } catch {}
		return this.connected;
	}

	disconnect(): void {
		this.ws?.close();
		this.ws = null;
		this.connected = false;
		this.interceptorInjected = false;
	}

	request(method: string, params: Record<string, unknown>): Promise<unknown> {
		return new Promise((resolve, reject) => {
			if (!this.ws) return reject(new Error("Not connected"));
			const id = this.cmdId++;
			this.pending.set(id, { resolve, reject });
			this.ws.send(JSON.stringify({ id, method, params }));
		});
	}

	async evaluate(expression: string): Promise<unknown> {
		const result = (await this.request("Runtime.evaluate", {
			expression,
			returnByValue: true,
			awaitPromise: false,
		})) as { result?: { value?: unknown }; exceptionDetails?: { text: string } };
		if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
		return result.result?.value;
	}

	on(event: string, handler: (params: unknown) => void): void {
		const list = this.eventHandlers.get(event) ?? [];
		list.push(handler);
		this.eventHandlers.set(event, list);
	}

	private onMessage(data: Buffer): void {
		let msg: { id?: number; result?: unknown; error?: { message: string }; method?: string; params?: unknown };
		try { msg = JSON.parse(data.toString()); } catch { return; }
		if (msg.id !== undefined) {
			const cb = this.pending.get(msg.id);
			if (cb) {
				this.pending.delete(msg.id);
				if (msg.error) cb.reject(new Error(msg.error.message));
				else cb.resolve(msg.result);
			}
		} else if (msg.method) {
			const handlers = this.eventHandlers.get(msg.method) ?? [];
			for (const h of handlers) h(msg.params);
		}
	}

	private async injectInterceptor(): Promise<void> {
		try {
			await this.evaluate(INTERCEPTOR);
			this.interceptorInjected = true;
		} catch {
			this.interceptorInjected = false;
		}
	}

	private onException(p: { exceptionDetails: { text: string; stackTrace?: { callFrames: Array<{ url: string; lineNumber: number; functionName: string }> } } }): void {
		store.addException({
			time: Date.now(),
			message: p.exceptionDetails.text ?? "Unknown exception",
			stack: (p.exceptionDetails.stackTrace?.callFrames ?? []).map(
				(f) => `${f.functionName || "<anon>"} @ ${f.url}:${f.lineNumber}`,
			),
		});
	}

	private onRequest(p: { requestId: string; request: { url: string; method: string }; timestamp: number }): void {
		store.addNetwork({
			id: p.requestId,
			url: p.request.url,
			method: p.request.method,
			startTime: p.timestamp * 1000,
			status: 0,
			failed: false,
		});
	}

	private onResponse(p: { requestId: string; response: { status: number; mimeType: string }; timestamp: number }): void {
		store.updateNetwork(p.requestId, { status: p.response.status, endTime: p.timestamp * 1000 });
	}

	private onFailed(p: { requestId: string; errorText: string; timestamp: number }): void {
		store.updateNetwork(p.requestId, { failed: true, errorText: p.errorText, endTime: p.timestamp * 1000 });
	}
}

export const cdp = new CdpClient();
