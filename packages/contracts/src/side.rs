/// Which machine an engine runs for: a player's App, or a Core's Minecraft server. Not the
/// client and server of a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Client,
    Server,
}
