use actix_web::web::Data;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use crossbeam_channel::{unbounded, Receiver, Sender};
use ffmpeg_next::{dictionary::Owned, Packet};
use serde::Deserialize;
use std::default::Default;
use std::env;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

mod ffmpeg;
mod filter;
use ffmpeg::StreamCtx;
use filter::FilterCtx;

#[derive(Default, Debug)]
struct TimeGap {
    pub audio_time: f64,
    pub video_time: f64,
}

fn ffmtrans(input_path: &Path, output_url: &Path, osd: &str, rx: Receiver<ThreadMsg>) {
    // init stream context
    let mut options = Owned::new();
    options.set("rtsp_transport", "tcp");
    options.set("max_delay", "500");
    let mut stream_ctx = StreamCtx::init(input_path, Some(options), output_url, "flv", None);
    let mut fmt_ctx = stream_ctx.fmt_ctx;

    // write header
    fmt_ctx
        .out_fmt_ctx
        .write_header()
        .expect("Failed to write header");
    // filter init
    let mut filter_ctx = FilterCtx::init_filter(&stream_ctx.dec_ctx, osd);
    // time gap init
    let mut time_gap = TimeGap::default();

    loop {
        if let Ok(msg) = rx.try_recv() {
            if msg.quit {
                break;
            }
        } else {
            println!("消息error")
        }
        //
        // if let Ok(msg) = rx.recv() {
        //     if msg.quit {
        //         break;
        //     }
        // } else {
        //     println!("消息error");
        // }
        let mut packet = Packet::empty();
        match packet.read(&mut fmt_ctx.in_fmt_ctx) {
            Ok(_) => {}
            Err(_) => {
                continue;
            }
        }
        if packet.size() == 0 {
            continue;
        }

        let stream_idx = packet.stream();
        // encoding video frame
        println!(
            "is video?:{};time_gap:{:#?}",
            stream_idx == stream_ctx.stream_idx.0 as usize,
            time_gap
        );
        if stream_idx == stream_ctx.stream_idx.0 as usize {
            let in_stream = fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap();
            packet.rescale_ts(in_stream.time_base(), stream_ctx.dec_ctx.time_base());

            // decode packet
            match stream_ctx.dec_ctx.send_packet(&packet) {
                Ok(()) => {
                    // println!("send_packet success");
                }
                Err(e) => {
                    // println!("send_packet failed:{e}");
                    continue;
                }
            };
            match stream_ctx.dec_ctx.receive_frame(&mut stream_ctx.de_frame) {
                Ok(()) => {
                    // println!("receive_frame success");
                }
                Err(e) => {
                    // println!("receive_frame failed:{}", e);
                    continue;
                }
            }
            let best_timestamp = stream_ctx.de_frame.timestamp();
            stream_ctx.de_frame.set_pts(best_timestamp);
            // println!(
            //     "packet.pts:{:?}deframe_pts:{:?}",
            //     packet.pts(),
            //     stream_ctx.de_frame.pts()
            // );
            filter_ctx.filter_encode_write_frame(
                &mut stream_ctx.de_frame,
                &mut stream_ctx.enc_ctx,
                &mut fmt_ctx,
                &mut time_gap,
            );
        } else {
            let out_fmt_timebase = fmt_ctx.out_fmt_ctx.stream(stream_idx).unwrap().time_base();
            packet.rescale_ts(
                fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap().time_base(),
                out_fmt_timebase,
            );

            let audio_time: f64 = packet.pts().unwrap() as f64 * f64::from(out_fmt_timebase);
            // println!("autio_time:{}s,pts:{:?}", audio_time, packet.pts());
            time_gap.audio_time = audio_time;
            packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        }
    }
}

#[derive(Deserialize, Debug)]
struct OSDReq {
    osd: String,
}

struct ThreadMsg {
    quit: bool,
}
#[derive(Clone)]
struct ThreadChannel {
    tx: Sender<ThreadMsg>,
    rx: Receiver<ThreadMsg>,
    pre_thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}

async fn trans_handler(data: Data<ThreadChannel>, body: web::Json<OSDReq>) -> HttpResponse {
    let mut thread_guard = data.pre_thread.lock().unwrap();

    if let Some(pre_thread) = thread_guard.take() {
        println!("Some!!!");
        data.tx
            .send(ThreadMsg { quit: true })
            .expect("send failed!!");
        pre_thread.join().unwrap();
    } else {
        println!("None!!!");
    }
    // get rx
    let rx = data.rx.clone();
    let new_thread = thread::spawn(move || {
        // Parse command line arguments
        let args: Vec<String> = env::args().collect();
        if args.len() != 3 {
            panic!("Usage: ./rtsp_to_rtmp <rtsp://stream-url> <rtmp://stream-url>");
        }
        let input_path = Path::new(&args[1]);
        let output_url = Path::new(&args[2]);
        println!("&body.osd:{}", &body.osd);
        // thread::sleep(std::time::Duration::from_secs(3));
        ffmtrans(input_path, output_url, &body.osd, rx);
    });
    *thread_guard = Some(new_thread);
    HttpResponse::Ok().body("ok")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // thread init
    let pre_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    // message
    let (tx, rx): (Sender<ThreadMsg>, Receiver<ThreadMsg>) = unbounded();
    // threadChannel
    let ThreadChannel = web::Data::new(ThreadChannel { tx, rx, pre_thread });
    // route
    HttpServer::new(move || {
        App::new()
            .app_data(ThreadChannel.clone())
            .route("/setosd", web::post().to(trans_handler))
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}
