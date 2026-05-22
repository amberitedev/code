/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as _socialRules from "../_socialRules.js";
import type * as auth from "../auth.js";
import type * as friendGroups from "../friendGroups.js";
import type * as friends from "../friends.js";
import type * as groupInvites from "../groupInvites.js";
import type * as http from "../http.js";
import type * as messaging from "../messaging.js";
import type * as presence from "../presence.js";
import type * as sync from "../sync.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  _socialRules: typeof _socialRules;
  auth: typeof auth;
  friendGroups: typeof friendGroups;
  friends: typeof friends;
  groupInvites: typeof groupInvites;
  http: typeof http;
  messaging: typeof messaging;
  presence: typeof presence;
  sync: typeof sync;
}>;

/**
 * A utility for referencing Convex functions in your app's public API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = api.myModule.myFunction;
 * ```
 */
export declare const api: FilterApi<
  typeof fullApi,
  FunctionReference<any, "public">
>;

/**
 * A utility for referencing Convex functions in your app's internal API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = internal.myModule.myFunction;
 * ```
 */
export declare const internal: FilterApi<
  typeof fullApi,
  FunctionReference<any, "internal">
>;

export declare const components: {};
