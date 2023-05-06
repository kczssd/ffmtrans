pub mod serve;
pub mod trans;

use crossbeam_channel::Receiver;
use ffmpeg_next::{dictionary::Owned, Packet};
use serve::route::ThreadMsg;
use std::{env, path::Path};
use trans::{ffmpeg::StreamCtx, filter::FilterCtx, sync::TimeGap};

pub fn ffmtrans_with_filter(osd: &str, rx: Receiver<ThreadMsg>) {
    // parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: ./rtsp_to_rtmp <rtsp://stream-url> <rtmp://stream-url>");
    }
    let input_path = Path::new(&args[1]);
    let output_url = Path::new(&args[2]);

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
        }
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

        if stream_idx == stream_ctx.stream_idx.0 as usize {
            let in_stream = fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap();
            packet.rescale_ts(in_stream.time_base(), stream_ctx.dec_ctx.time_base());

            // decode packet
            match stream_ctx.dec_ctx.send_packet(&packet) {
                Ok(()) => {
                    // println!("send_packet success");
                }
                Err(_) => {
                    // println!("send_packet failed:{}");
                    continue;
                }
            };
            match stream_ctx.dec_ctx.receive_frame(&mut stream_ctx.de_frame) {
                Ok(()) => {
                    // println!("receive_frame success");
                }
                Err(_) => {
                    // println!("receive_frame failed:{}");
                    continue;
                }
            }
            let best_timestamp = stream_ctx.de_frame.timestamp();
            stream_ctx.de_frame.set_pts(best_timestamp);

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
            time_gap.audio_time = audio_time;
            packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        }
    }
}

pub fn ffmtrans_remux(rx: Receiver<ThreadMsg>) {
    // parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: ./rtsp_to_rtmp <rtsp://stream-url> <rtmp://stream-url>");
    }
    let input_path = Path::new(&args[1]);
    let output_url = Path::new(&args[2]);

    // init stream context
    let mut options = Owned::new();
    options.set("rtsp_transport", "tcp");
    options.set("max_delay", "500");
    let stream_ctx = StreamCtx::init(input_path, Some(options), output_url, "flv", None);
    let mut fmt_ctx = stream_ctx.fmt_ctx;

    // write header
    fmt_ctx
        .out_fmt_ctx
        .write_header()
        .expect("Failed to write header");

    // time gap init
    let mut time_gap = TimeGap::default();

    loop {
        if let Ok(msg) = rx.try_recv() {
            if msg.quit {
                break;
            }
        }

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

        // remux
        if stream_idx == stream_ctx.stream_idx.0 as usize {
            let in_stream = fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap();
            packet.rescale_ts(in_stream.time_base(), stream_ctx.dec_ctx.time_base());
            packet.set_stream(0);
            let out_fmt_timebase = fmt_ctx.out_fmt_ctx.stream(0).unwrap().time_base();
            unsafe {
                packet.rescale_ts((*stream_ctx.enc_ctx.as_ptr()).time_base, out_fmt_timebase);
            }
            packet.set_dts(Some(
                (time_gap.audio_time / f64::from(out_fmt_timebase)) as i64,
            ));
            packet.set_pts(Some(
                (time_gap.audio_time / f64::from(out_fmt_timebase)) as i64,
            ));
            let video_time: f64 = packet.pts().unwrap() as f64 * f64::from(out_fmt_timebase);
            time_gap.video_time = video_time;
            packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        } else {
            let out_fmt_timebase = fmt_ctx.out_fmt_ctx.stream(stream_idx).unwrap().time_base();
            packet.rescale_ts(
                fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap().time_base(),
                out_fmt_timebase,
            );

            let audio_time: f64 = packet.pts().unwrap() as f64 * f64::from(out_fmt_timebase);
            time_gap.audio_time = audio_time;
            packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        }
    }
}
