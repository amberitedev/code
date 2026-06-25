import { httpRouter } from "convex/server";
import { auth } from "./auth";
import { syncSnapshot as coreProjectionSync } from "./coreProjection";
import { handle as realtimeBridge } from "./realtimeBridge";

const http = httpRouter();

auth.addHttpRoutes(http);
http.route({ path: "/realtime/bridge", method: "POST", handler: realtimeBridge });
http.route({ path: "/core/projection-sync", method: "POST", handler: coreProjectionSync });

export default http;
