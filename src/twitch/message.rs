#[derive(Debug, Clone)]
pub struct TwitchChatMessage {
    pub username: String,
    pub message: String,
}

impl TwitchChatMessage {
    pub fn parse(line: &str) -> Option<Self> {
        let privmsg = line.find(" PRIVMSG ")?;
        let prefix = &line[..privmsg];
        let remainder = &line[privmsg + " PRIVMSG ".len()..];
        let message_start = remainder.find(" :")?;
        let message = &remainder[message_start + 2..];
        let username_start = prefix.rfind(':')? + 1;
        let username_end = prefix[username_start..].find('!')? + username_start;

        Some(Self {
            username: prefix[username_start..username_end].to_string(),
            message: message.trim_end().to_string(),
        })
    }
}
