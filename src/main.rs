extern crate ws;
extern crate serde_json;
extern crate redis;
use redis::Commands;

use std::rc::Rc;

mod message;
use message::Message;
use std::collections::HashMap;
use crate::message::RelayKey;
use std::cell::RefCell;

#[derive(Debug, PartialEq, Clone)]
pub enum Session {
    NotConnected,
    ClientRegistered { socket: Rc<ws::Sender> },
    AppStreamRegistered { socket: Rc<ws::Sender> },
    PendingValidation {
        client: Rc<ws::Sender>,
        app_stream: Rc<ws::Sender>,
    },
    Established {
        client: Rc<ws::Sender>,
        app_stream: Rc<ws::Sender>,
    },
    Failure(String),
}

#[derive(Debug, Clone)]
enum SessionEvent {
    RegisterClient { socket: Rc<ws::Sender> },
    RegisterAppStream { socket: Rc<ws::Sender> },
//    Validate,
//    Invalidate,
}


impl Session {
    fn next(&self, event: &SessionEvent) -> Session {
        match (self, event) {
            (Session::NotConnected, SessionEvent::RegisterClient { socket }) => {
                Session::ClientRegistered { socket: Rc::clone(socket) }
            },
            (Session::NotConnected, SessionEvent::RegisterAppStream { socket}) => {
                Session::AppStreamRegistered { socket: Rc::clone(socket) }
            },
            (Session::ClientRegistered { socket: client }, SessionEvent::RegisterAppStream { socket: app_stream }) => {
                Session::PendingValidation {
                    client: Rc::clone(client),
                    app_stream: Rc::clone( app_stream),
                }
            },
            (Session::AppStreamRegistered { socket: app_stream }, SessionEvent::RegisterClient { socket: client }) => {
                Session::PendingValidation {
                    client: Rc::clone(client),
                    app_stream: Rc::clone( app_stream),
                }
            }
            (s, e) => {
                Session::Failure(format!("Wrong state, event combination: {:#?} {:#?}", s, e))
            }
        }
    }
}

pub struct Server {
    out: Rc<ws::Sender>,
    session: Session,
    sessions: Rc<RefCell<HashMap<RelayKey, u32>>>,
}

impl ws::Handler for Server {
    fn on_open(&mut self, shake: ws::Handshake) -> ws::Result<()> {
        if let Some(ip_addr) = shake.remote_addr()? {
            println!("Connection opened from {}.", ip_addr)
        } else {
            println!("Unable to obtain client's IP address.")
        }
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
//        println!("Server got message '{}' from {:?}. ", msg, self.out.token());
        println!("{:?}", self.session);
        let result : Result<Message, serde_json::Error> = serde_json::from_str(msg.as_text().unwrap());
        match result {
            Ok(value) => {
                match value {
                    Message::Ping {} => self.out.send(serde_json::to_string(&Message::Pong {}).unwrap()),
                    Message::ClientRegister {data} => {
                        println!("{:?}", data.key);
                        self.sessions.borrow_mut().insert(data.key, 0);
                        let event = SessionEvent::RegisterClient { socket: Rc::clone(&self.out) };
                        self.session = self.session.next(&event);
                        Ok(())
                    },
                    Message::AppStreamRegister {data} => {
                        self.sessions.borrow_mut().insert(data.key, 1);
                        let event = SessionEvent::RegisterAppStream { socket: Rc::clone(&self.out) };
                        self.session = self.session.next(&event);
                        Ok(())
                    },
                    _ => self.out.close(ws::CloseCode::Unsupported),
                }
            },
            Err(_error) => self.out.close(ws::CloseCode::Invalid),
        }
    }

    fn on_close(&mut self, code: ws::CloseCode, reason: &str) {
        println!("WebSocket closing for ({:?}) {}", code, reason);
    }
}

fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let con = client.get_connection().unwrap();
    let _ : () = con.set("my_key", 42).unwrap();
    let r : i32 = con.get("my_key").unwrap();
    println!("{}", r);

    let sessions: Rc<RefCell<HashMap<RelayKey, u32>>> = Rc::new(RefCell::new(HashMap::new()));

    if let Err(error) = ws::listen("127.0.0.1:3012", |out| {
        Server {
            out: Rc::new(out),
            session: Session::NotConnected,
            sessions: Rc::clone(&sessions),
        }
    }) {
        println!("Failed to create WebSocket due to {:?}", error);
    }
}
