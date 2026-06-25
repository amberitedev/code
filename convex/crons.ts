import { cronJobs } from "convex/server";
import { internal } from "./_generated/api";

const crons = cronJobs();

crons.interval(
	"cleanup expired realtime bridge requests",
	{ hours: 24 },
	internal.realtimeBridge.cleanupExpiredRequests,
);

export default crons;
