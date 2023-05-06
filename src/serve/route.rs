use actix_web::web::Data;
use actix_web::{web, HttpResponse};
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::{ffmtrans_remux, ffmtrans_with_filter};

#[derive(Deserialize, Debug)]
pub struct OSDReq {
    osd: String,
}

pub struct ThreadMsg {
    pub quit: bool,
}

#[derive(Clone)]
pub struct ThreadChannel {
    pub tx: Sender<ThreadMsg>,
    pub rx: Receiver<ThreadMsg>,
    pub pre_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}
impl ThreadChannel {
    pub fn new() -> Self {
        // pre_thread init
        let pre_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
        // message
        let (tx, rx): (Sender<ThreadMsg>, Receiver<ThreadMsg>) = unbounded();
        // threadChannel
        ThreadChannel { tx, rx, pre_thread }
    }
}

pub async fn trans_handler(data: Data<ThreadChannel>, body: web::Json<OSDReq>) -> HttpResponse {
    let mut thread_guard = data.pre_thread.lock().unwrap();

    if let Some(pre_thread) = thread_guard.take() {
        data.tx
            .send(ThreadMsg { quit: true })
            .expect("send failed!!");
        pre_thread.join().unwrap();
    }

    // get rx clone
    let rx = data.rx.clone();

    let new_thread = thread::spawn(move || match &body.osd.len() {
        0 => ffmtrans_remux(rx),
        _ => ffmtrans_with_filter(&body.osd, rx),
    });

    *thread_guard = Some(new_thread);
    HttpResponse::Ok().body("ok")
}
