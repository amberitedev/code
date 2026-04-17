import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

serve(async (req: Request) => {
  try {
    const { connection_code, core_secret, player_count, is_online, game_version } = await req.json();

    if (!connection_code || !core_secret) {
      return new Response("Missing connection_code or core_secret", { status: 400 });
    }

    // Service role key is set in Supabase dashboard env vars
    const supabaseClient = createClient(
      Deno.env.get("SUPABASE_URL") ?? "",
      Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? ""
    );

    // Look up the core by connection code
    const { data: core, error: fetchError } = await supabaseClient
      .from("cores")
      .select("id, core_secret_hash")
      .eq("connection_code", connection_code)
      .single();

    if (fetchError || !core) {
      return new Response("Core not found", { status: 404 });
    }

    // Verify SHA-256 hash of the provided secret matches stored hash
    const encoder = new TextEncoder();
    const hashBuffer = await crypto.subtle.digest("SHA-256", encoder.encode(core_secret));
    const hashHex = Array.from(new Uint8Array(hashBuffer))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");

    if (hashHex !== core.core_secret_hash) {
      return new Response("Unauthorized", { status: 401 });
    }

    // Update the cores table with status
    const updateData: any = {
      is_online,
      player_count,
      last_seen: new Date().toISOString(),
    };
    if (game_version) {
      updateData.game_version = game_version;
    }

    const { error: updateError } = await supabaseClient
      .from("cores")
      .update(updateData)
      .eq("id", core.id);

    if (updateError) {
      console.error("Failed to update core status:", updateError);
      return new Response("Failed to update status", { status: 500 });
    }

    return new Response(JSON.stringify({ success: true }), {
      headers: { "Content-Type": "application/json" },
      status: 200,
    });
  } catch (error) {
    console.error("Error handling heartbeat:", error);
    return new Response("Internal server error", { status: 500 });
  }
});
