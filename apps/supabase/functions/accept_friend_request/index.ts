import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

serve(async (req: Request) => {
  try {
    const authHeader = req.headers.get("Authorization");
    if (!authHeader) {
      return new Response("Missing authorization header", { status: 401 });
    }

    const supabaseClient = createClient(
      Deno.env.get("SUPABASE_URL") ?? "",
      Deno.env.get("SUPABASE_ANON_KEY") ?? "",
      {
        global: {
          headers: { Authorization: authHeader },
        },
      }
    );

    const {
      data: { user },
      error: userError,
    } = await supabaseClient.auth.getUser();

    if (userError || !user) {
      return new Response("Unauthorized", { status: 401 });
    }

    const recipientId = user.id;
    const { sender_id } = await req.json();

    if (!sender_id) {
      return new Response("Missing sender_id", { status: 400 });
    }

    const { data: request, error: fetchError } = await supabaseClient
      .from("friend_requests")
      .select("*")
      .eq("sender_id", sender_id)
      .eq("recipient_id", recipientId)
      .single();

    if (fetchError || !request) {
      return new Response("Friend request not found", { status: 404 });
    }

    const { error: deleteError } = await supabaseClient
      .from("friend_requests")
      .delete()
      .eq("sender_id", sender_id)
      .eq("recipient_id", recipientId);

    if (deleteError) {
      return new Response("Failed to accept request", { status: 500 });
    }

    const { error: insertError } = await supabaseClient
      .from("friendships")
      .insert({ user_id_a: sender_id, user_id_b: recipientId });

    if (insertError) {
      return new Response("Failed to create friendship", { status: 500 });
    }

    return new Response(JSON.stringify({ success: true }), {
      headers: { "Content-Type": "application/json" },
      status: 200,
    });
  } catch (error) {
    console.error("Error accepting friend request:", error);
    return new Response("Internal server error", { status: 500 });
  }
});
