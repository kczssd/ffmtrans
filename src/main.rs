use ffmpeg_next::dictionary::Owned;
use filter::OSD;
use std::env;
use std::path::Path;

mod ffmpeg;
mod filter;
use ffmpeg::FFmpeg;

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
    let mut ffmpeg = FFmpeg::init(input_path, output_url);

    // Open input file using FFmpeg
    let mut options = Owned::new();
    // options.set("rtsp_transport", "tcp");
    // options.set("max_delay", "550000");
    ffmpeg.in_open(Some(options));

    // Open output URL using FFmpeg
    ffmpeg.out_open("mp4", None); // udp_fmt: mpegts

    // init filter;
    // ffmpeg.init_filter();
    ffmpeg.decoder();
    ffmpeg.encoder();
    // remuxer output with input_format_context
    // ffmpeg.remuxer_stream();
}
