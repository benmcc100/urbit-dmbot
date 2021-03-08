pub use urbit_http_api::chat::{AuthoredMessage, Message};
use urbit_http_api::{default_cli_ship_interface_setup, Node, ShipInterface};
use std::{thread, time};

/// This struct represents a chatbot that is connected to a given `ship`,
/// is watching for DM requests to `ship`, accepting them, and is 
/// using the function `respond_to_message` to process any messages
/// which are posted in said DM conversation.
pub struct DMbot {
    /// `respond_to_message` is a function defined by the user of this framework.
    /// This function receives any DM messages sent to the ship, by anyone,
    /// and if the function returns `Some(message)`, then `message` is posted to the
    /// chat as a response. If it returns `None`, then no message is posted.
    respond_to_message: fn(AuthoredMessage) -> Option<Message>,
    ship: ShipInterface,
}

impl DMbot {
    /// Create a new `DMbot` with a manually provided `ShipInterface`
    pub fn new(
        respond_to_message: fn(AuthoredMessage) -> Option<Message>,
        ship: ShipInterface,
    ) -> Self {
        DMbot {
            respond_to_message: respond_to_message,
            ship: ship,
        }
    }

    /// Create a new `DMbot` with a `ShipInterface` derived automatically
    /// from a local config file. If the config file does not exist, the
    /// `DMbot` will create the config file, exit, and prompt the user to
    /// fill it out.
    pub fn new_with_local_config(
        respond_to_message: fn(AuthoredMessage) -> Option<Message>,
    ) -> Self {
        let ship = default_cli_ship_interface_setup();
        Self::new(respond_to_message, ship)
    }

    /// Run the `DMbot`
    pub fn run(&self) -> Option<()> {
        println!("=======================================\nPowered By The Urbit Chatbot Framework\n=======================================");
        // Create a `Subscription`
        let mut channel = self.ship.create_channel().unwrap();
        // Subscribe to all graph-store updates
        channel
            .create_new_subscription("graph-store", "/updates")
            .unwrap();
        // Subscribe to invite-store to see DM invites to the ship
        channel
            .create_new_subscription("invite-store", "/updates")
            .unwrap();
        // Create second channel over which to send pokes
        let mut poke_channel = self.ship.create_channel().unwrap();
        // Generate name of our DMs
        let dm_name = &format!("dm--{}", self.ship.ship_name);

        // Infinitely watch for new graph store updates
        loop {
            thread::sleep(time::Duration::from_millis(1000));
            channel.parse_event_messages();
            let mut messages_to_send = vec![];
            let invite_updates = channel.find_subscription("invite-store", "/updates")?;

            loop {
                // See if there are invites to DMs
                let invites = invite_updates.pop_message();
                if let Some(mess) = &invites {
                    if let Ok(invite_json) = json::parse(mess) {
                        // Push all these blocks of code into DM struct in http-api
                        // Check if invite_update is an actual invite, ignore if not
                        if !(&format!("{}", &invite_json["invite-update"]["invite"]["invite"]["resource"]["name"].clone()) == dm_name){
                            continue;
                        }
                        // Poke group-view to subscribe ship to DM hosted on ship trying to talk to us
                        let inviting_ship = format!("~{}", invite_json["invite-update"]["invite"]["invite"]["resource"]["ship"].clone());
                        let mut poke_data = json::JsonValue::new_object();
                        poke_data["join"] = json::JsonValue::new_object();
                        poke_data["join"]["resource"] = json::JsonValue::new_object();
                        poke_data["join"]["resource"]["ship"] = inviting_ship.clone().into();
                        poke_data["join"]["resource"]["name"] = dm_name.clone().into();
                        poke_data["join"]["ship"] = inviting_ship.clone().into();
                        // Handle Ok response vs not-Ok response in http-api
                        let _poke_response = poke_channel.poke("group-view", "group-view-action", &poke_data);
                        //// Poke invite-store to accept DM invite
                        let mut poke2_data = json::JsonValue::new_object();
                        poke2_data["accept"] = json::JsonValue::new_object();
                        poke2_data["accept"]["term"] = "graph".to_string().into();
                        poke2_data["accept"]["uid"] = invite_json["invite-update"]["invite"]["uid"].clone().into();
                        let _poke2_response = poke_channel.poke("invite-store", "invite-action", &poke_data);
                        // Poke hark-store to remove invite notification
                        // Something else needs to be done here to truly resolve the
                        // notification in Landscape, I still haven't found it...
                        let mut poke3_data = json::JsonValue::new_object();
                        poke3_data["seen"] = json::Null;
                        let _poke3_response = poke_channel.poke("hark-store", "hark-action", &poke_data);
                    }
                }
                // If no invites left, stop
                if let None = &invites {
                    break;
                }
            }

            let graph_updates = channel.find_subscription("graph-store", "/updates")?;
            // Read all of the current SSE messages to find if any new DMs
            loop {
                let pop_res = graph_updates.pop_message();
                // Acquire the message
                if let Some(mess) = &pop_res {
                    // Parse it to json
                    if let Ok(json) = json::parse(mess) {
                        // see if this message is a DM to us
                        if !(&format!("{}", &json["graph-update"]["add-nodes"]["resource"]["name"].clone()) == dm_name){
                            continue;
                        }
                        // parse json to a `Node`
                        if let Ok(node) = Node::from_graph_update_json(&json) {
                            // If the message is posted by the DMbot ship, ignore
                            if node.author == self.ship.ship_name {
                                continue;
                            }
                            let authored_message = AuthoredMessage::new(node.author, node.contents);
                            // If the DMbot intends to respond to the provided message
                            if let Some(message) = (self.respond_to_message)(authored_message) {
                                println!("Replied to message.");
                                let reply_to = &json["graph-update"]["add-nodes"]["resource"]["ship"].clone();
                                messages_to_send.push((reply_to.to_string(), message))
                            }
                        }
                    }
                }
                // If no messages left, stop
                if let None = &pop_res {
                    break;
                }
            }

            // Send each response message that was returned by the `respond_to_message`
            // function. This is separated until after done parsing messages due to mutable borrows.
            // For DMs, since the ship messaging us always initiates and this hosts the graph,
            // the ship-name we pass here is theirs.
            for message in messages_to_send {
                channel
                    .chat()
                    .send_message(&format!("~{}", &message.0), dm_name, &message.1)
                    .ok();
            }
        }
    }
}
