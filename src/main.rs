use discord_sdk::activity::{ActivityBuilder, Assets};
use discord_sdk::user::User;
use discord_sdk::{Discord};
use parking_lot as pl;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio;
use tokio::task::JoinHandle;

struct UserMngr {
    connected: Arc<pl::RwLock<bool>>,
    con_since: Arc<pl::RwLock<SystemTime>>,
    stop: Arc<pl::RwLock<bool>>,
    dsc: Arc<tokio::sync::RwLock<Discord>>, // tokio rwlock implements send
    u: Arc<pl::RwLock<Option<User>>>,
}

impl UserMngr {
    pub fn new(dsc: Discord) -> Self {
        Self {
            connected: Arc::new(Default::default()),
            stop: Arc::new(Default::default()),
            u: Arc::new(Default::default()),
            dsc: Arc::new(tokio::sync::RwLock::new(dsc)),
            con_since: Arc::new(pl::RwLock::new(SystemTime::now())),
        }
    }

    pub fn stop(&mut self) {
        *self.stop.clone().write() = true;
    }

    pub fn start_activity_update(&self) -> JoinHandle<()> {
        let stopper = self.stop.clone();
        let con = self.connected.clone();
        let cons = self.con_since.clone();
        let dscc = self.dsc.clone();
        tokio::task::spawn(async move {
            let mut s = 0;
            let mut last_update = SystemTime::now();
            loop {
                let new_time = SystemTime::now();
                if new_time.duration_since(last_update).unwrap_or(Duration::from_secs(1)) > Duration::from_secs(20) {
                    println!("probably slept");
                    *cons.clone().write() = new_time.clone();
                };
                last_update = new_time;
                tokio::time::sleep(Duration::from_secs(5)).await;
                let w = *stopper.clone().read();
                // println!("stop : {w}");
                if w {
                    break;
                }
                if s == 4 {
                    // 0 <-> 5 = 4
                    if !*con.clone().read() {
                        println!("no connect");
                        continue;
                    }
                    println!("update status");

                    let since = { cons.clone().read().clone() };

                    let activity = ActivityBuilder::default()
                        .details("on mac")
                        .state("pogging")
                        .assets(
                            Assets::default().large("mon".to_owned(), Some("Monterey".to_owned())),
                        )
                        .start_timestamp(since);
                    {
                        match Arc::clone(&dscc).write().await.update_activity(activity).await{
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("unable to update status: {}",e)
                            }
                        };
                    }
                }
                s = (s + 1) % 5;
            }
        })
    }
}

#[tokio::main(worker_threads = 5)]
async fn main() {
    let discord_app = discord_sdk::DiscordApp::PlainId(1019026970010451979);
    let (wheel, handlr) = discord_sdk::wheel::Wheel::new(Box::new(|err| {
        println!("{}", err);
    }));

    let mut user = wheel.user();

    let subs = discord_sdk::Subscriptions::ACTIVITY;

    let dsc =
        Discord::new(discord_app, subs, Box::new(handlr)).expect("unable to get dsc");
    let mngr = UserMngr::new(dsc);

    let joiner = mngr.start_activity_update();
    let ac_mngr = Arc::new(pl::RwLock::new(mngr));

    let ac2 = ac_mngr.clone();
    ctrlc::set_handler(move || {
        println!("stopping");
        ac2.write().stop();
    })
    .expect("Error setting Ctrl-C handler");
    let mut inter = tokio::time::interval(Duration::from_secs(5));

    println!("now waiting for discord to respond");

    loop {
        tokio::select! {
            _ = inter.tick() => {

                let ac = ac_mngr.read();
                if *ac.stop.read() {
                    break;
                }
            }
            _= user.0.changed() => {
                let _user = match &*user.0.borrow() {
                    discord_sdk::wheel::UserState::Connected(user) => {
                        let u = user.clone();
                        let ac = ac_mngr.write();

                        println!("connected to {}",u.username);

                        *ac.u.clone().write() = Some(u);
                        *ac.con_since.clone().write() = SystemTime::now();
                        *ac.connected.clone().write() = true
                    },
                    discord_sdk::wheel::UserState::Disconnected(err) => {

                        let ac = ac_mngr.write();

                        if *ac.connected.clone().read() {
                            eprintln!("disconnected from user reason : {}",err);
                            *ac.u.clone().write() = None;
                            *ac.connected.clone().write() = false
                        }

                    },
                };
            }
        }
    }

    ac_mngr.write().stop();

    joiner.await.expect("unable to join status updater");

    // ac_mngr.write().await.dsc.write().await..disconnect().await;

    println!("bye")
}
