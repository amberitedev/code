import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { cdp } from "./cdp-client.js";
import * as status from "./tools/status.js";
import * as console_ from "./tools/console.js";
import * as errors from "./tools/errors.js";
import * as network from "./tools/network.js";
import * as performance from "./tools/performance.js";
import * as page from "./tools/page.js";

const server = new McpServer({
	name: "tari-devtools",
	version: "0.1.0",
});

status.register(server);
console_.register(server);
errors.register(server);
network.register(server);
performance.register(server);
page.register(server);

// Connect CDP in background — tools return helpful error if not yet connected
cdp.connect().catch(() => {
	// Will retry on next tool call if needed; tools check cdp.connected
});

const transport = new StdioServerTransport();
await server.connect(transport);
