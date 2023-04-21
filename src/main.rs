use ffmpeg_next::{dictionary::Owned, Packet};
use ffmpeg_sys_next::AV_TIME_BASE;
use std::env;
use std::path::Path;

mod ffmpeg;
mod filter;
use ffmpeg::StreamCtx;
use filter::FilterCtx;

fn main() {
    // Parse command line arguments
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
    let mut filter_ctx = FilterCtx::init_filter(&stream_ctx.dec_ctx);
    let frame_idx = 0;
    loop {
        let mut packet = Packet::empty();
        match packet.read(&mut fmt_ctx.in_fmt_ctx) {
            Ok(_) => {}
            Err(e) => {
                continue;
            }
        }
        if packet.size() == 0 {
            continue;
        }

        let stream_idx = packet.stream();
        // encoding video frame
        if stream_idx == stream_ctx.stream_idx.0 as usize {
            // set pts dts duration
            if packet.pts().is_none() || packet.dts().is_none() {
                // write pts
                let stream = fmt_ctx
                    .in_fmt_ctx
                    .stream(stream_ctx.stream_idx.0 as usize)
                    .unwrap();
                let time_base = stream.time_base();
                // duration between 2 frames
                let duration = AV_TIME_BASE as i64 / f64::from(stream.rate()) as i64;
                packet.set_pts(Some(
                    ((frame_idx * duration) as f64 / (f64::from(time_base) * AV_TIME_BASE as f64))
                        as i64,
                ));
                packet.set_dts(packet.pts());
                packet.set_duration(
                    (duration as f64 / (f64::from(time_base) * AV_TIME_BASE as f64)) as i64,
                )
            }
            let in_stream = fmt_ctx.in_fmt_ctx.stream(packet.stream()).unwrap();
            let out_stream = fmt_ctx.out_fmt_ctx.stream(packet.stream()).unwrap();
            packet.rescale_ts(in_stream.time_base(), out_stream.time_base());
            packet.set_position(-1);

            // decode packet
            match stream_ctx.dec_ctx.send_packet(&packet) {
                Ok(()) => {
                    println!("send_packet success");
                }
                Err(e) => {
                    println!("send_packet failed:{e}");
                    continue;
                }
            };
            match stream_ctx.dec_ctx.receive_frame(&mut stream_ctx.de_frame) {
                Ok(()) => {
                    println!("receive_frame success");
                }
                Err(e) => {
                    println!("receive_frame failed:{}", e);
                    continue;
                }
            }
            filter_ctx.filter_encode_write_frame(
                &mut stream_ctx.de_frame,
                &mut stream_ctx.enc_ctx,
                &mut fmt_ctx,
            );

            // plan 1: without filter
            // match stream_ctx.enc_ctx.send_frame(&stream_ctx.de_frame) {
            //     Ok(()) => {
            //         println!("send_frame success");
            //     }
            //     Err(e) => {
            //         println!("send_frame failed:{}", e);
            //         continue;
            //     }
            // }
            // let mut re_packet = Packet::empty();
            // match stream_ctx.enc_ctx.receive_packet(&mut re_packet) {
            //     Ok(()) => {
            //         println!("receive_packet success");
            //     }
            //     Err(e) => {
            //         println!("receive_packet failed:{}", e);
            //         continue;
            //     }
            // }
            // re_packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        } else {
            packet.rescale_ts(
                fmt_ctx.in_fmt_ctx.stream(stream_idx).unwrap().time_base(),
                fmt_ctx.out_fmt_ctx.stream(stream_idx).unwrap().time_base(),
            );
            packet.write(&mut fmt_ctx.out_fmt_ctx).unwrap();
        }
    }
}
