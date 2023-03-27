use ffmpeg_next::{
    dictionary::Owned,
    format,
    time::{self, current},
    Packet, Rational, Rescale, Rounding,
};
use ffmpeg_sys_next::AV_TIME_BASE;
use std::env;
use std::path::Path;

mod ffmpeg;
mod filter;
use ffmpeg::{FmtCtx, StreamCtx};
use filter::FilterCtx;

fn main() {
    // ffmpeg -re -stream_loop -1 -i testmv.mp4 -c:v copy -c:a copy -rtsp_transport tcp -r 30 -b 2000k -s 1080x720 -f rtsp rtsp://localhost:8554/stream
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Usage: ./mp4_to_rtmp <filename.mp4> <rtmp://stream-url>");
    }
    let input_path = Path::new(&args[1]);
    let output_url = Path::new(&args[2]);
    // ffmpeg init
    format::register_all();
    format::network::init();
    //
    let mut stream_ctx = StreamCtx::new();
    let mut options = Owned::new();
    options.set("rtsp_transport", "tcp");
    options.set("max_delay", "550000");
    stream_ctx.in_open(input_path, Some(options));
    stream_ctx.out_open(output_url, "flv", None);
    let mut fmt_ctx = stream_ctx.fmt_ctx;

    let frame_idx = 0;
    let start_time = current();
    loop {
        let mut packet = Packet::empty();
        match packet.read(fmt_ctx.in_fmt_ctx.as_mut().unwrap()) {
            Ok(_) => {}
            Err(_) => break,
        } // pts dts duration
        if packet.pts().is_none() || packet.dts().is_none() {
            //Write PTS
            let stream = fmt_ctx
                .in_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(stream_ctx.stream_idx.0 as usize)
                .unwrap();
            let time_base = stream.time_base();
            //Duration between 2 frames (us)
            let duration = AV_TIME_BASE as i64 / f64::from(stream.rate()) as i64;
            //Parameters
            packet.set_pts(Some(
                ((frame_idx * duration) as f64 / (f64::from(time_base) * AV_TIME_BASE as f64))
                    as i64,
            ));
            packet.set_dts(packet.pts());
            packet.set_duration(
                (duration as f64 / (f64::from(time_base) * AV_TIME_BASE as f64)) as i64,
            )
        }
        let stream_idx = packet.stream();
        println!(
            "is video?{},{}",
            stream_idx, stream_ctx.stream_idx.0 as usize
        );
        // re-encoding video
        if stream_idx == stream_ctx.stream_idx.0 as usize {
            // delay
            let time_base = fmt_ctx
                .in_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(stream_ctx.stream_idx.0 as usize)
                .unwrap()
                .time_base();
            let time_base_q = Rational::new(1, AV_TIME_BASE);
            let pts_time = packet.dts().unwrap().rescale(time_base, time_base_q);
            let now_time = current() - start_time;
            if pts_time > now_time {
                time::sleep((pts_time - now_time) as u32).expect("Failed to sleep");
            }
            let in_stream = fmt_ctx
                .in_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(packet.stream())
                .unwrap();
            let out_stream = fmt_ctx
                .out_fmt_ctx
                .as_ref()
                .unwrap()
                .stream(packet.stream())
                .unwrap();
            // copy packet
            packet.set_pts(Some(packet.pts().unwrap().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            )));
            packet.set_dts(Some(packet.dts().unwrap().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            )));
            packet.set_duration(packet.duration().rescale_with(
                in_stream.time_base(),
                out_stream.time_base(),
                Rounding::NearInfinity,
            ));
            packet.set_position(-1);

            packet.rescale_ts(
                fmt_ctx
                    .in_fmt_ctx
                    .as_ref()
                    .unwrap()
                    .stream(stream_idx)
                    .unwrap()
                    .time_base(),
                stream_ctx.dec_ctx.0.as_ref().unwrap().time_base(),
            );

            let dec_ctx = stream_ctx.dec_ctx.0.as_mut().unwrap();
            match dec_ctx.send_packet(&packet) {
                Ok(()) => {}
                Err(e) => {
                    println!("{e:?}");
                    break;
                }
            };
            // filter init
            let mut filter_ctx = FilterCtx::default();
            filter_ctx.init_filter(dec_ctx);
            loop {
                match dec_ctx.receive_frame(stream_ctx.de_frame.0.as_mut().unwrap()) {
                    Ok(()) => {}
                    Err(e) => {
                        println!("{}", e);
                        break;
                    }
                }
                let de_frame = stream_ctx.de_frame.0.as_mut().unwrap();
                let timestamp = de_frame.timestamp();
                de_frame.set_pts(timestamp);
                filter_ctx.filter_encode_write_frame(
                    de_frame,
                    stream_ctx.enc_ctx.0.as_mut().unwrap(),
                    &mut fmt_ctx,
                );
            }
        }
        // else {
        //     packet.rescale_ts(
        //         fmt_ctx
        //             .in_fmt_ctx
        //             .as_ref()
        //             .unwrap()
        //             .stream(stream_idx)
        //             .unwrap()
        //             .time_base(),
        //         fmt_ctx
        //             .out_fmt_ctx
        //             .as_ref()
        //             .unwrap()
        //             .stream(stream_idx)
        //             .unwrap()
        //             .time_base(),
        //     );
        //     packet.write(fmt_ctx.out_fmt_ctx.as_mut().unwrap()).unwrap();
        // }
    }
}
