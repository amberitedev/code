import { httpRouter } from "convex/server";
import { auth } from "./auth";
import { handle as realtimeBridge } from "./realtimeBridge";

const http = httpRouter();

auth.addHttpRoutes(http);
http.route({ path: "/realtime/bridge", method: "POST", handler: realtimeBridge });

export default http;
