#![feature(let_chains)]
use std::borrow::Cow;
use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::{Arc};
use tokio::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use discord_sdk::activity::{Activity, ActivityBuilder, Assets};
use discord_sdk::Discord;
use discord_sdk::user::User;
use tokio;
use tokio::task::JoinHandle;


struct UserMngr {
    connected: Arc<RwLock<bool>>,
    con_since: Arc<RwLock<SystemTime>>,
    stop: Arc<RwLock<bool>>,
    dsc : Arc<RwLock<Discord>>,
    u: Arc<RwLock<Option<User>>>,
}

impl UserMngr {
    pub fn new(dsc:Discord) -> Self {
        Self {
            connected:Arc::new(Default::default()),
            stop:Arc::new(Default::default()),
            u: Arc::new(Default::default()),
            dsc: Arc::new(RwLock::new(dsc)),
            con_since: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    pub async fn stop(&mut self) {
        *self.stop.clone().write().await = true;
    }

    pub fn start_activity_update<'a>(&'a self) -> JoinHandle<()> {
        let stopper = self.stop.clone();
        let con = self.connected.clone();
        let cons = self.con_since.clone();
        let dscc = self.dsc.clone();
        tokio::task::spawn( async move {
            let mut s = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let w = *stopper.clone().read().await;
                // println!("stop : {w}");
                if w {
                    break;
                }
                if s == 4 { // 0 <-> 5 = 4
                    if !*con.clone().read().await {
                        continue;
                    }
                    // println!("update status");

                    let since = {
                        cons.clone().read().await.clone()
                    };

                    let activity = ActivityBuilder::default()
                        .details("on mac")
                        .state("pogging")
                        .assets(Assets::default().large("mon".to_owned(),Some("Monterey".to_owned())))
                        .start_timestamp(since);


                    Arc::clone(&dscc).write().await.update_activity(activity).await;

                }
                s= (s + 1) % 5;
            }
        })
    }

}

#[tokio::main(worker_threads = 5)]
async fn main() {
    let discord_app = discord_sdk::DiscordApp::PlainId(1019026970010451979);
    let (wheel,handlr) = discord_sdk::wheel::Wheel::new(Box::new(|err| {
        println!("{}",err);
    }));

    let mut user = wheel.user();

    let subs = discord_sdk::Subscriptions::ACTIVITY;

    let dsc = discord_sdk::Discord::new(discord_app,subs,Box::new(handlr)).expect("unable to get dsc");
    let mngr = UserMngr::new(dsc);

    let joiner = mngr.start_activity_update();
    let mut ac_mngr = Arc::new(RwLock::new(mngr));

    let ac2 = ac_mngr.clone();
    ctrlc::set_handler(move || {
        println!("stopping");
        futures::executor::block_on(async {
            ac2.write().await.stop().await;
        })

    })
    .expect("Error setting Ctrl-C handler");
    let mut inter = tokio::time::interval(Duration::from_secs(5));

    println!("now waiting for discord to respond");

    loop {

        tokio::select!{
            _ = inter.tick() => {

                let mut ac = ac_mngr.read().await;
                if *ac.stop.read().await {
                    break;
                }
            }
            _= user.0.changed() => {
                let user = match &*user.0.borrow() {
                    discord_sdk::wheel::UserState::Connected(user) => {
                        let u = user.clone();
                        let mut ac = ac_mngr.write().await;

                        println!("connected to {}",u.username);

                        *ac.u.clone().write().await = Some(u);
                        *ac.con_since.clone().write().await = SystemTime::now();
                        *ac.connected.clone().write().await = true
                    },
                    discord_sdk::wheel::UserState::Disconnected(err) => {

                        let mut ac = ac_mngr.write().await;

                        if *ac.connected.clone().read().await {
                            eprintln!("disconnected from user");
                            *ac.u.clone().write().await = None;
                            *ac.connected.clone().write().await = false
                        }

                    },
                };
            }
        }
    }

    ac_mngr.write().await.stop();

    joiner.await;

    // ac_mngr.write().await.dsc.write().await..disconnect().await;

    println!("bye")

}