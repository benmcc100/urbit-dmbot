use urbit_chatbot_framework::{AuthoredMessage, DMbot, Message};

fn respond_to_message(_authored_message: AuthoredMessage) -> Option<Message> {
    // Any time a message is posted in the chat, respond in chat with a static message.
    Some(Message::new().add_text("Calm Computing ~"))
}

fn main() {
    let dm_bot = DMbot::new_with_local_config(respond_to_message);
    dm_bot.run();
}
