use std::env;
use std::path::Path;

mod ffmpeg;
use ffmpeg::FFmpeg;

fn main() {
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
    ffmpeg.in_open(None);

    // Open output URL using FFmpeg
    ffmpeg.out_open("flv", None); // udp_fmt: mpegts

    // remuxer output with input_format_context
    ffmpeg.remuxer();
}
